mod operators;
mod control_flow;
mod calls;
mod collections;

use inkwell::values::BasicMetadataValueEnum;

use crate::parser::ast::{
    AccessExpr, AssignExpr, AssignOp, BinaryExpr, BinaryOp,
    CallExpr, Expr, ExprKind, ForExpr, IfExpr, IndexExpr,
    Literal, MethodCallExpr, NewExpr, PostfixOp, TypeName,
    UnaryOp, VectorExpr,
};
use crate::semantic::HulkType;

use super::context::CodegenContext;
use super::error::{CodegenError, CodegenResult};
use super::symbols::Place;
use super::value::{CgValue, ELEM_SIZE_BYTES};
use super::visitor::ExprVisitor;

// ── Helpers de evaluación ──────────────────────────────────────────────────────

impl<'ctx> CodegenContext<'ctx> {
    pub(super) fn eval_number(
        &mut self,
        expr: &Expr,
    ) -> CodegenResult<inkwell::values::FloatValue<'ctx>> {
        let value = self.visit_expr(expr)?;
        self.require_number(value)
    }

    pub(super) fn eval_bool(
        &mut self,
        expr: &Expr,
    ) -> CodegenResult<inkwell::values::IntValue<'ctx>> {
        let value = self.visit_expr(expr)?;
        self.require_bool(value)
    }

    pub(super) fn eval_lvalue_slot<'a>(
        &'a self,
        expr: &Expr,
    ) -> CodegenResult<&'a Place<'ctx>> {
        match &expr.kind {
            ExprKind::Identifier { name } => self
                .symbols
                .get(name)
                .ok_or_else(|| CodegenError::UnknownVariable(name.clone())),
            _ => Err(CodegenError::InvalidLValue),
        }
    }

    pub(super) fn get_expr_type(&self, expr: &Expr) -> CodegenResult<HulkType> {
        self.expr_types.get(&expr.id)
            .cloned()
            .ok_or_else(|| CodegenError::Unsupported(
                format!("expresión {} sin anotación de tipo — fallo del TypeChecker", expr.id)))
    }
}

// ── Visitor principal — despacha a métodos en los submódulos ──────────────────

impl<'ctx> ExprVisitor<'ctx> for CodegenContext<'ctx> {
    fn visit_expr(&mut self, expr: &Expr) -> CodegenResult<CgValue<'ctx>> {
        match &expr.kind {
            // ── Literales ─────────────────────────────────────────────────────
            ExprKind::Literal(lit) => match lit {
                Literal::Number { value, .. } => {
                    let parsed: f64 = value
                        .parse()
                        .map_err(|_| CodegenError::ParseNumber(value.clone()))?;
                    Ok(CgValue::Number(self.f64_type().const_float(parsed)))
                }
                Literal::Bool { value, .. } => {
                    Ok(CgValue::Bool(self.bool_type().const_int(*value as u64, false)))
                }
                Literal::Null { .. } => Ok(CgValue::Null),
                Literal::String { value, .. } => {
                    let ptr = self.builder
                        .build_global_string_ptr(value, "str")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?
                        .as_pointer_value();
                    Ok(CgValue::Str(ptr))
                }
                Literal::Char { value, .. } => {
                    let ptr = self.builder
                        .build_global_string_ptr(value, "chr")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?
                        .as_pointer_value();
                    Ok(CgValue::Str(ptr))
                }
            },

            // ── Identificador ─────────────────────────────────────────────────
            ExprKind::Identifier { name } => {
                let slot = self
                    .symbols
                    .get(name)
                    .ok_or_else(|| CodegenError::UnknownVariable(name.clone()))?
                    .clone();
                self.load_place(&slot, &format!("load_{name}"))
            }

            // ── Operaciones ───────────────────────────────────────────────────
            ExprKind::Binary(bin) => self.lower_binary(bin),
            ExprKind::Assign(ae)  => self.lower_assign(ae),

            // ── Unarias ───────────────────────────────────────────────────────
            ExprKind::Unary(unary) => match unary.op {
                UnaryOp::Neg => {
                    let value = self.eval_number(&unary.operand)?;
                    Ok(CgValue::Number(
                        self.builder.build_float_neg(value, "negtmp")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?,
                    ))
                }
                UnaryOp::Not => {
                    let value = self.eval_bool(&unary.operand)?;
                    Ok(CgValue::Bool(
                        self.builder.build_not(value, "nottmp")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?,
                    ))
                }
            },

            // ── Postfix (++/--) ───────────────────────────────────────────────
            ExprKind::Postfix(postfix) => {
                let slot = self.eval_lvalue_slot(&postfix.operand)?.clone();
                let old = self.builder
                    .build_load(self.f64_type(), slot.ptr, "post_load")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .into_float_value();
                let delta = self.f64_type().const_float(1.0);
                let new_val = match postfix.op {
                    PostfixOp::Increment => self.builder
                        .build_float_add(old, delta, "post_inc")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?,
                    PostfixOp::Decrement => self.builder
                        .build_float_sub(old, delta, "post_dec")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?,
                };
                self.builder.build_store(slot.ptr, new_val)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                Ok(CgValue::Number(old))
            }

            // ── Control de flujo ──────────────────────────────────────────────
            ExprKind::If(if_expr) => self.lower_if(if_expr),
            ExprKind::While(we) => {
                let function  = self.current_fn()?;
                let cond_block = self.context.append_basic_block(function, "while_cond");
                let body_block = self.context.append_basic_block(function, "while_body");
                let end_block  = self.context.append_basic_block(function, "while_end");

                self.builder.build_unconditional_branch(cond_block)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;

                self.builder.position_at_end(cond_block);
                let cond = self.eval_bool(&we.condition)?;
                self.builder.build_conditional_branch(cond, body_block, end_block)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;

                self.builder.position_at_end(body_block);
                let _ = self.visit_expr(&we.body)?;
                if !self.is_current_block_terminated() {
                    self.builder.build_unconditional_branch(cond_block)
                        .map_err(|e| CodegenError::Builder(e.to_string()))?;
                }

                self.builder.position_at_end(end_block);
                Ok(CgValue::Void)
            }
            ExprKind::For(fe) => self.lower_for(fe),

            // ── Llamadas ──────────────────────────────────────────────────────
            ExprKind::Call(call) => self.lower_call(call),

            // ── Bloque / Let ──────────────────────────────────────────────────
            ExprKind::Block(block) => {
                let mut last = CgValue::Void;
                for e in &block.body {
                    if self.is_current_block_terminated() { break; }
                    last = self.visit_expr(e)?;
                }
                Ok(last)
            }
            ExprKind::Let(let_expr) => {
                let function = self.current_fn()?;
                self.push_scope();
                for binding in &let_expr.bindings {
                    let val  = self.visit_expr(&binding.value)?;
                    let ty   = val.hulk_type();
                    let slot = self.create_entry_alloca_for(function, &binding.name, &ty)?;
                    self.store_place(&slot, val)?;
                    self.symbols.insert(binding.name.clone(), slot);
                }
                let out = self.visit_expr(&let_expr.body)?;
                self.pop_scope();
                Ok(out)
            }

            // ── Objetos / Colecciones ─────────────────────────────────────────
            ExprKind::New(new_expr) => {
                let type_name = new_expr.type_name.name().to_string();
                let ctor_name = format!("__hulk_ctor_{}", type_name);
                let ctor_fn   = self.module.get_function(&ctor_name)
                    .ok_or_else(|| CodegenError::UnknownFunction(ctor_name.clone()))?;
                let ctor_param_types: Vec<HulkType> = self.type_hierarchy.types
                    .get(&type_name)
                    .map(|ti| ti.constructor_params.iter().map(|(_, t)| t.clone()).collect())
                    .unwrap_or_default();
                let mut args: Vec<BasicMetadataValueEnum<'ctx>> = vec![];
                for (i, arg_expr) in new_expr.args.iter().enumerate() {
                    let val      = self.visit_expr(arg_expr)?;
                    let expected = ctor_param_types.get(i).cloned().unwrap_or(HulkType::Number);
                    args.push(self.coerce_arg(val, &expected)?);
                }
                let obj_ptr = self.builder
                    .build_call(ctor_fn, &args, "new_obj")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .try_as_basic_value().left()
                    .ok_or_else(|| CodegenError::Unsupported(
                        format!("constructor de '{}' no retornó valor", type_name)))?
                    .into_pointer_value();
                Ok(CgValue::Object(obj_ptr))
            }
            ExprKind::Access(ae) => {
                let obj_val = self.visit_expr(&ae.object)?;
                let obj_ptr = match obj_val {
                    CgValue::Object(p) => p,
                    _ => return Err(CodegenError::Unsupported(
                        format!("access '{}': el receptor no es un objeto", ae.field))),
                };
                let type_name = match self.get_expr_type(&ae.object)? {
                    HulkType::UserDefined(n) => n,
                    _ => return Err(CodegenError::Unsupported(
                        format!("access '{}': tipo receptor no es UserDefined", ae.field))),
                };
                let place = self.field_place(obj_ptr, &type_name, &ae.field)?;
                self.load_place(&place, &ae.field)
            }
            ExprKind::MethodCall(mc) => self.lower_method_call(mc, expr),
            ExprKind::Index(ie) => {
                let coll_val = self.visit_expr(&ie.collection)?;
                let CgValue::Vector(vec_ptr) = coll_val else {
                    return Err(CodegenError::Unsupported(
                        "Index: la colección no es un Vector".into()));
                };
                let idx_val = self.visit_expr(&ie.index)?;
                let idx_f64 = self.require_number(idx_val)?;
                let i32_ty  = self.context.i32_type();
                let idx_i32 = self.builder
                    .build_float_to_signed_int(idx_f64, i32_ty, "idx_i32")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                let get_fn   = self.require_fn("hulk_vec_get")?;
                let elem_ptr = self.builder
                    .build_call(get_fn,
                        &[vec_ptr.into(),
                          idx_i32.into(),
                          i32_ty.const_int(ELEM_SIZE_BYTES, false).into()], "ep")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .try_as_basic_value().left()
                    .ok_or_else(|| CodegenError::Unsupported("hulk_vec_get sin retorno".into()))?
                    .into_pointer_value();
                let elem_ty = match self.get_expr_type(&ie.collection)? {
                    HulkType::Vector(t) => *t,
                    _                   => HulkType::Object,
                };
                self.load_place(&Place { ptr: elem_ptr, hulk_ty: elem_ty }, "elem")
            }
            ExprKind::Is { expr: inner, type_name } => self.lower_is(inner, type_name),
            ExprKind::As { expr: inner, .. }        => self.visit_expr(inner),
            ExprKind::Vector(ve)                    => self.lower_vector(ve),
            ExprKind::Base => Err(CodegenError::Unsupported(
                "base aun no implementado".to_string())),
        }
    }
}
