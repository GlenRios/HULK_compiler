use inkwell::FloatPredicate;
use inkwell::IntPredicate;
use inkwell::module::Linkage;
use inkwell::types::BasicType;
use inkwell::values::{BasicMetadataValueEnum, BasicValue};

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
    fn eval_number(&mut self, expr: &Expr) -> CodegenResult<inkwell::values::FloatValue<'ctx>> {
        let value = self.visit_expr(expr)?;
        self.require_number(value)
    }

    fn eval_bool(&mut self, expr: &Expr) -> CodegenResult<inkwell::values::IntValue<'ctx>> {
        let value = self.visit_expr(expr)?;
        self.require_bool(value)
    }

    fn eval_lvalue_slot<'a>(&'a self, expr: &Expr) -> CodegenResult<&'a Place<'ctx>> {
        match &expr.kind {
            ExprKind::Identifier { name } => self
                .symbols
                .get(name)
                .ok_or_else(|| CodegenError::UnknownVariable(name.clone())),
            _ => Err(CodegenError::InvalidLValue),
        }
    }

    fn get_expr_type(&self, expr: &Expr) -> CodegenResult<HulkType> {
        self.expr_types.get(&expr.id)
            .cloned()
            .ok_or_else(|| CodegenError::Unsupported(
                format!("expresión {} sin anotación de tipo — fallo del TypeChecker", expr.id)))
    }
}

// ── Visitor principal — despacha a métodos privados ────────────────────────────

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
            ExprKind::Binary(bin)      => self.lower_binary(bin),
            ExprKind::Assign(ae)       => self.lower_assign(ae),

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
                let function = self.current_fn()?;
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
                let ctor_fn = self.module.get_function(&ctor_name)
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
                    .try_as_basic_value().left().unwrap()
                    .into_pointer_value();
                Ok(CgValue::Object(obj_ptr))
            }
            ExprKind::Access(ae) => {
                let CgValue::Object(obj_ptr) = self.visit_expr(&ae.object)? else {
                    unreachable!("objeto UserDefined produjo un CgValue que no es Object")
                };
                let HulkType::UserDefined(type_name) = self.get_expr_type(&ae.object)? else {
                    unreachable!("access sobre tipo no UserDefined: el TypeChecker debería haber rechazado esto")
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
                    .try_as_basic_value().left().unwrap()
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
            ExprKind::Base => Err(CodegenError::Unsupported("base aun no implementado".to_string())),
        }
    }
}

// ── Métodos privados de lowering ───────────────────────────────────────────────

impl<'ctx> CodegenContext<'ctx> {
    fn lower_binary(&mut self, bin: &BinaryExpr) -> CodegenResult<CgValue<'ctx>> {
        match &bin.op {
            BinaryOp::Add => {
                let l = self.eval_number(&bin.left)?;
                let r = self.eval_number(&bin.right)?;
                Ok(CgValue::Number(
                    self.builder.build_float_add(l, r, "addtmp")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?,
                ))
            }
            BinaryOp::Sub => {
                let l = self.eval_number(&bin.left)?;
                let r = self.eval_number(&bin.right)?;
                Ok(CgValue::Number(
                    self.builder.build_float_sub(l, r, "subtmp")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?,
                ))
            }
            BinaryOp::Mul => {
                let l = self.eval_number(&bin.left)?;
                let r = self.eval_number(&bin.right)?;
                Ok(CgValue::Number(
                    self.builder.build_float_mul(l, r, "multmp")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?,
                ))
            }
            BinaryOp::Div => {
                let l = self.eval_number(&bin.left)?;
                let r = self.eval_number(&bin.right)?;
                Ok(CgValue::Number(
                    self.builder.build_float_div(l, r, "divtmp")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?,
                ))
            }
            BinaryOp::Mod => {
                let l = self.eval_number(&bin.left)?;
                let r = self.eval_number(&bin.right)?;
                Ok(CgValue::Number(
                    self.builder.build_float_rem(l, r, "modtmp")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?,
                ))
            }
            BinaryOp::Power => {
                let l = self.eval_number(&bin.left)?;
                let r = self.eval_number(&bin.right)?;
                let pow_fn = self.module.get_function("pow").unwrap_or_else(|| {
                    let f64_t = self.f64_type();
                    let ty = f64_t.fn_type(&[f64_t.into(), f64_t.into()], false);
                    self.module.add_function("pow", ty, Some(Linkage::External))
                });
                let result = self
                    .builder
                    .build_call(pow_fn, &[l.into(), r.into()], "powtmp")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .try_as_basic_value()
                    .left()
                    .ok_or_else(|| CodegenError::Unsupported("pow no retorno valor".to_string()))?
                    .into_float_value();
                Ok(CgValue::Number(result))
            }
            BinaryOp::Eq | BinaryOp::NotEq => {
                let is_eq = matches!(bin.op, BinaryOp::Eq);
                match self.get_expr_type(&bin.left)? {
                    HulkType::Boolean => {
                        let l = self.eval_bool(&bin.left)?;
                        let r = self.eval_bool(&bin.right)?;
                        let pred = if is_eq { IntPredicate::EQ } else { IntPredicate::NE };
                        Ok(CgValue::Bool(
                            self.builder.build_int_compare(pred, l, r, "eqtmp")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
                    }
                    HulkType::StringT => {
                        let lv = self.visit_expr(&bin.left)?;
                        let rv = self.visit_expr(&bin.right)?;
                        let lp = self.cgvalue_to_str(lv)?;
                        let rp = self.cgvalue_to_str(rv)?;
                        let f  = self.require_fn("hulk_str_eq")?;
                        let result = self.builder
                            .build_call(f, &[lp.into(), rp.into()], "streq")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?
                            .try_as_basic_value().left().unwrap().into_int_value();
                        let out = if is_eq {
                            result
                        } else {
                            self.builder.build_not(result, "strne")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?
                        };
                        Ok(CgValue::Bool(out))
                    }
                    HulkType::UserDefined(_) | HulkType::Object => {
                        let lv = self.visit_expr(&bin.left)?;
                        let rv = self.visit_expr(&bin.right)?;
                        let i64_t = self.context.i64_type();
                        let lp = match lv {
                            CgValue::Object(p) | CgValue::Str(p) | CgValue::Vector(p) => p,
                            CgValue::Null => self.ptr_type().const_null(),
                            _ => return Err(CodegenError::Unsupported("eq sobre tipo no-puntero".into())),
                        };
                        let rp = match rv {
                            CgValue::Object(p) | CgValue::Str(p) | CgValue::Vector(p) => p,
                            CgValue::Null => self.ptr_type().const_null(),
                            _ => return Err(CodegenError::Unsupported("eq sobre tipo no-puntero".into())),
                        };
                        let li = self.builder.build_ptr_to_int(lp, i64_t, "lptr")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?;
                        let ri = self.builder.build_ptr_to_int(rp, i64_t, "rptr")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?;
                        let pred = if is_eq { IntPredicate::EQ } else { IntPredicate::NE };
                        Ok(CgValue::Bool(
                            self.builder.build_int_compare(pred, li, ri, "ptreq")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
                    }
                    _ => {
                        let l = self.eval_number(&bin.left)?;
                        let r = self.eval_number(&bin.right)?;
                        let pred = if is_eq { FloatPredicate::OEQ } else { FloatPredicate::ONE };
                        Ok(CgValue::Bool(
                            self.builder.build_float_compare(pred, l, r, "eqtmp")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
                    }
                }
            }
            BinaryOp::Less => {
                let l = self.eval_number(&bin.left)?;
                let r = self.eval_number(&bin.right)?;
                Ok(CgValue::Bool(
                    self.builder.build_float_compare(FloatPredicate::OLT, l, r, "lttmp")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?,
                ))
            }
            BinaryOp::Greater => {
                let l = self.eval_number(&bin.left)?;
                let r = self.eval_number(&bin.right)?;
                Ok(CgValue::Bool(
                    self.builder.build_float_compare(FloatPredicate::OGT, l, r, "gttmp")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?,
                ))
            }
            BinaryOp::LessEq => {
                let l = self.eval_number(&bin.left)?;
                let r = self.eval_number(&bin.right)?;
                Ok(CgValue::Bool(
                    self.builder.build_float_compare(FloatPredicate::OLE, l, r, "letmp")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?,
                ))
            }
            BinaryOp::GreaterEq => {
                let l = self.eval_number(&bin.left)?;
                let r = self.eval_number(&bin.right)?;
                Ok(CgValue::Bool(
                    self.builder.build_float_compare(FloatPredicate::OGE, l, r, "getmp")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?,
                ))
            }
            BinaryOp::And => {
                let fn_val      = self.current_fn()?;
                let rhs_block   = self.context.append_basic_block(fn_val, "and_rhs");
                let merge_block = self.context.append_basic_block(fn_val, "and_merge");

                let lhs       = self.eval_bool(&bin.left)?;
                let lhs_block = self.builder.get_insert_block().unwrap();
                self.builder.build_conditional_branch(lhs, rhs_block, merge_block)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;

                self.builder.position_at_end(rhs_block);
                let rhs           = self.eval_bool(&bin.right)?;
                let rhs_end_block = self.builder.get_insert_block().unwrap();
                self.builder.build_unconditional_branch(merge_block)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;

                self.builder.position_at_end(merge_block);
                let phi       = self.builder.build_phi(self.bool_type(), "andtmp")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                let false_val = self.bool_type().const_int(0, false);
                phi.add_incoming(&[
                    (&false_val as &dyn BasicValue<'ctx>, lhs_block),
                    (&rhs       as &dyn BasicValue<'ctx>, rhs_end_block),
                ]);
                Ok(CgValue::Bool(phi.as_basic_value().into_int_value()))
            }
            BinaryOp::Or => {
                let fn_val      = self.current_fn()?;
                let rhs_block   = self.context.append_basic_block(fn_val, "or_rhs");
                let merge_block = self.context.append_basic_block(fn_val, "or_merge");

                let lhs       = self.eval_bool(&bin.left)?;
                let lhs_block = self.builder.get_insert_block().unwrap();
                self.builder.build_conditional_branch(lhs, merge_block, rhs_block)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;

                self.builder.position_at_end(rhs_block);
                let rhs           = self.eval_bool(&bin.right)?;
                let rhs_end_block = self.builder.get_insert_block().unwrap();
                self.builder.build_unconditional_branch(merge_block)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;

                self.builder.position_at_end(merge_block);
                let phi      = self.builder.build_phi(self.bool_type(), "ortmp")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                let true_val = self.bool_type().const_int(1, false);
                phi.add_incoming(&[
                    (&true_val as &dyn BasicValue<'ctx>, lhs_block),
                    (&rhs      as &dyn BasicValue<'ctx>, rhs_end_block),
                ]);
                Ok(CgValue::Bool(phi.as_basic_value().into_int_value()))
            }
            BinaryOp::Concat => {
                let l = self.visit_expr(&bin.left)?;
                let r = self.visit_expr(&bin.right)?;
                let lp = self.cgvalue_to_str(l)?;
                let rp = self.cgvalue_to_str(r)?;
                let f  = self.require_fn("hulk_str_concat")?;
                let ptr = self.builder
                    .build_call(f, &[lp.into(), rp.into()], "concat")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .try_as_basic_value().left().unwrap().into_pointer_value();
                Ok(CgValue::Str(ptr))
            }
            BinaryOp::DoubleConcat => {
                let l = self.visit_expr(&bin.left)?;
                let r = self.visit_expr(&bin.right)?;
                let lp = self.cgvalue_to_str(l)?;
                let rp = self.cgvalue_to_str(r)?;
                let f  = self.require_fn("hulk_str_concat_space")?;
                let ptr = self.builder
                    .build_call(f, &[lp.into(), rp.into()], "dconcat")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .try_as_basic_value().left().unwrap().into_pointer_value();
                Ok(CgValue::Str(ptr))
            }
        }
    }

    fn lower_assign(&mut self, assign: &AssignExpr) -> CodegenResult<CgValue<'ctx>> {
        let place = match &assign.target.kind {
            ExprKind::Identifier { .. } => {
                self.eval_lvalue_slot(&assign.target)?.clone()
            }
            ExprKind::Access(ae) => {
                let CgValue::Object(obj_ptr) = self.visit_expr(&ae.object)? else {
                    unreachable!("lvalue Access: receptor no es Object")
                };
                let HulkType::UserDefined(type_name) = self.get_expr_type(&ae.object)? else {
                    unreachable!("lvalue Access: tipo no es UserDefined")
                };
                self.field_place(obj_ptr, &type_name, &ae.field)?
            }
            _ => return Err(CodegenError::InvalidLValue),
        };

        let rhs = self.visit_expr(&assign.value)?;

        match assign.op {
            AssignOp::Assign => {
                self.store_place(&place, rhs)?;
                Ok(rhs)
            }
            AssignOp::PlusAssign => {
                let old    = self.require_number(self.load_place(&place, "old")?)?;
                let rhs_f  = self.require_number(rhs)?;
                let result = self.builder.build_float_add(old, rhs_f, "plus_assign")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                self.store_place(&place, CgValue::Number(result))?;
                Ok(CgValue::Number(result))
            }
            AssignOp::MinusAssign => {
                let old    = self.require_number(self.load_place(&place, "old")?)?;
                let rhs_f  = self.require_number(rhs)?;
                let result = self.builder.build_float_sub(old, rhs_f, "minus_assign")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                self.store_place(&place, CgValue::Number(result))?;
                Ok(CgValue::Number(result))
            }
            AssignOp::MulAssign => {
                let old    = self.require_number(self.load_place(&place, "old")?)?;
                let rhs_f  = self.require_number(rhs)?;
                let result = self.builder.build_float_mul(old, rhs_f, "mul_assign")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                self.store_place(&place, CgValue::Number(result))?;
                Ok(CgValue::Number(result))
            }
            AssignOp::DivAssign => {
                let old    = self.require_number(self.load_place(&place, "old")?)?;
                let rhs_f  = self.require_number(rhs)?;
                let result = self.builder.build_float_div(old, rhs_f, "div_assign")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                self.store_place(&place, CgValue::Number(result))?;
                Ok(CgValue::Number(result))
            }
            AssignOp::ModAssign => {
                let old    = self.require_number(self.load_place(&place, "old")?)?;
                let rhs_f  = self.require_number(rhs)?;
                let result = self.builder.build_float_rem(old, rhs_f, "mod_assign")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                self.store_place(&place, CgValue::Number(result))?;
                Ok(CgValue::Number(result))
            }
        }
    }

    fn lower_call(&mut self, call: &CallExpr) -> CodegenResult<CgValue<'ctx>> {
        // ── base(args) — llamada directa al método del padre ──────────────────
        if let ExprKind::Base = &call.callee.kind {
            let method_name = self.current_method_name.clone()
                .ok_or_else(|| CodegenError::Unsupported(
                    "base() fuera de contexto de método".into()))?;
            let type_name = self.current_type_name.clone()
                .ok_or_else(|| CodegenError::Unsupported(
                    "base() fuera de contexto de tipo".into()))?;

            let parent_name = self.type_hierarchy.types.get(&type_name)
                .and_then(|ti| ti.parent.clone())
                .ok_or_else(|| CodegenError::Unsupported(
                    format!("tipo '{}' no tiene padre", type_name)))?;

            let impl_type = self.type_hierarchy
                .find_method_impl_type(&parent_name, &method_name)
                .unwrap_or(parent_name);

            let sig = self.type_hierarchy.types
                .get(&impl_type)
                .and_then(|ti| ti.methods.get(&method_name))
                .cloned()
                .ok_or_else(|| CodegenError::Unsupported(
                    format!("método '{}' no encontrado en '{}'", method_name, impl_type)))?;

            let self_ptr = self.self_ptr
                .ok_or_else(|| CodegenError::Unsupported("base() sin self_ptr".into()))?;
            let mut call_args: Vec<BasicMetadataValueEnum<'ctx>> = vec![self_ptr.into()];
            for (i, arg_expr) in call.args.iter().enumerate() {
                let val      = self.visit_expr(arg_expr)?;
                let expected = sig.params.get(i)
                    .map(|(_, t)| t.clone())
                    .unwrap_or(HulkType::Object);
                call_args.push(self.coerce_arg(val, &expected)?);
            }

            let static_fn_name = format!("__hulk_method_{}_{}", impl_type, method_name);
            let fn_val = self.module.get_function(&static_fn_name)
                .ok_or_else(|| CodegenError::UnknownFunction(static_fn_name.clone()))?;

            let result = self.builder
                .build_call(fn_val, &call_args, "base_result")
                .map_err(|e| CodegenError::Builder(e.to_string()))?;

            return match &sig.return_type {
                HulkType::Number  => Ok(CgValue::Number(
                    result.try_as_basic_value().left()
                        .ok_or_else(|| CodegenError::Unsupported("base() sin retorno".into()))?
                        .into_float_value()
                )),
                HulkType::Boolean => Ok(CgValue::Bool(
                    result.try_as_basic_value().left()
                        .ok_or_else(|| CodegenError::Unsupported("base() sin retorno".into()))?
                        .into_int_value()
                )),
                HulkType::StringT => Ok(CgValue::Str(
                    result.try_as_basic_value().left()
                        .ok_or_else(|| CodegenError::Unsupported("base() sin retorno".into()))?
                        .into_pointer_value()
                )),
                HulkType::Null | HulkType::Unknown => Ok(CgValue::Null),
                _ => Ok(CgValue::Object(
                    result.try_as_basic_value().left()
                        .ok_or_else(|| CodegenError::Unsupported("base() sin retorno".into()))?
                        .into_pointer_value()
                )),
            };
        }

        // ── llamada normal por nombre ─────────────────────────────────────────
        let callee_name = match &call.callee.kind {
            ExprKind::Identifier { name } => name.clone(),
            _ => return Err(CodegenError::Unsupported(
                "solo se soporta llamada directa por nombre".to_string(),
            )),
        };

        // ── Builtins especiales ───────────────────────────────────────────────
        match callee_name.as_str() {
            "print" => {
                let arg_val = self.visit_expr(&call.args[0])?;
                let str_ptr = self.cgvalue_to_str(arg_val)?;
                let print_fn = self.require_fn("hulk_print")?;
                self.builder
                    .build_call(print_fn, &[str_ptr.into()], "")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                return Ok(CgValue::Null);
            }
            "rand" => {
                let f = self.require_fn("hulk_rand")?;
                let v = self.builder
                    .build_call(f, &[], "randtmp")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .try_as_basic_value().left().unwrap().into_float_value();
                return Ok(CgValue::Number(v));
            }
            "sqrt" | "sin" | "cos" | "exp" => {
                let arg = self.eval_number(&call.args[0])?;
                let f = self.require_fn(&callee_name)?;
                let v = self.builder
                    .build_call(f, &[arg.into()], "mathtmp")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .try_as_basic_value().left().unwrap().into_float_value();
                return Ok(CgValue::Number(v));
            }
            "log" => {
                let base = self.eval_number(&call.args[0])?;
                let val  = self.eval_number(&call.args[1])?;
                let ln_fn = self.require_fn("log")?;
                let ln_val = self.builder
                    .build_call(ln_fn, &[val.into()],  "ln_val")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .try_as_basic_value().left().unwrap().into_float_value();
                let ln_base = self.builder
                    .build_call(ln_fn, &[base.into()], "ln_base")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .try_as_basic_value().left().unwrap().into_float_value();
                let result = self.builder
                    .build_float_div(ln_val, ln_base, "log_result")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                return Ok(CgValue::Number(result));
            }
            "range" => {
                let start    = self.eval_number(&call.args[0])?;
                let end      = self.eval_number(&call.args[1])?;
                let alloc_fn = self.require_fn("hulk_range_alloc")?;
                let ptr = self.builder
                    .build_call(alloc_fn, &[start.into(), end.into()], "range_obj")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .try_as_basic_value().left().unwrap()
                    .into_pointer_value();
                return Ok(CgValue::Object(ptr));
            }
            _ => {}
        }

        // ── Llamada general ───────────────────────────────────────────────────
        let function = self.functions.get(&callee_name).copied()
            .or_else(|| self.module.get_function(&callee_name))
            .ok_or_else(|| CodegenError::UnknownFunction(callee_name.clone()))?;

        let sig = self.func_sigs.get(&callee_name).cloned();

        let mut args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(call.args.len());
        for (i, arg) in call.args.iter().enumerate() {
            let val      = self.visit_expr(arg)?;
            let expected = sig.as_ref()
                .and_then(|s| s.params.get(i))
                .map(|(_, t)| t.clone())
                .unwrap_or(HulkType::Object);
            args.push(self.coerce_arg(val, &expected)?);
        }

        let call_site = self.builder
            .build_call(function, &args, "calltmp")
            .map_err(|e| CodegenError::Builder(e.to_string()))?;

        let ret_ty = sig.map(|s| s.return_type).unwrap_or(HulkType::Number);
        Ok(self.call_result_to_cgvalue(call_site, &ret_ty))
    }

    fn lower_if(&mut self, if_expr: &IfExpr) -> CodegenResult<CgValue<'ctx>> {
        let function = self.current_fn()?;
        let merge_block = self.context.append_basic_block(function, "if_merge");

        let mut incoming: Vec<(CgValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> = Vec::new();

        // rama if
        {
            let then_block = self.context.append_basic_block(function, "if_then");
            let next_block = if if_expr.elif_chain.is_empty() {
                self.context.append_basic_block(function, "if_else")
            } else {
                self.context.append_basic_block(function, "elif_0_cond")
            };
            let cond = self.eval_bool(&if_expr.condition)?;
            self.builder.build_conditional_branch(cond, then_block, next_block)
                .map_err(|e| CodegenError::Builder(e.to_string()))?;
            self.builder.position_at_end(then_block);
            let val = self.visit_expr(&if_expr.then_body)?;
            let end = self.builder.get_insert_block()
                .ok_or_else(|| CodegenError::Unsupported("if_then sin bloque".to_string()))?;
            if !self.is_current_block_terminated() {
                self.builder.build_unconditional_branch(merge_block)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                incoming.push((val, end));
            }
            self.builder.position_at_end(next_block);
        }

        // ramas elif
        for (i, elif) in if_expr.elif_chain.iter().enumerate() {
            let then_block = self.context.append_basic_block(function, &format!("elif_{i}_then"));
            let is_last = i + 1 == if_expr.elif_chain.len();
            let next_block = if is_last {
                self.context.append_basic_block(function, "if_else")
            } else {
                self.context.append_basic_block(function, &format!("elif_{}_cond", i + 1))
            };
            let cond = self.eval_bool(&elif.condition)?;
            self.builder.build_conditional_branch(cond, then_block, next_block)
                .map_err(|e| CodegenError::Builder(e.to_string()))?;
            self.builder.position_at_end(then_block);
            let val = self.visit_expr(&elif.body)?;
            let end = self.builder.get_insert_block()
                .ok_or_else(|| CodegenError::Unsupported(format!("elif_{i}_then sin bloque")))?;
            if !self.is_current_block_terminated() {
                self.builder.build_unconditional_branch(merge_block)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                incoming.push((val, end));
            }
            self.builder.position_at_end(next_block);
        }

        // rama else
        {
            let val = self.visit_expr(&if_expr.else_body)?;
            let end = self.builder.get_insert_block()
                .ok_or_else(|| CodegenError::Unsupported("if_else sin bloque".to_string()))?;
            if !self.is_current_block_terminated() {
                self.builder.build_unconditional_branch(merge_block)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                incoming.push((val, end));
            }
        }

        self.builder.position_at_end(merge_block);

        // Si ninguna rama fluye al merge, el bloque es inalcanzable
        if incoming.is_empty() {
            self.builder.build_unreachable()
                .map_err(|e| CodegenError::Builder(e.to_string()))?;
            return Ok(CgValue::Void);
        }

        // PHI tipado según el tipo de la primera rama que fluye al merge
        match &incoming[0].0 {
            CgValue::Bool(_) => {
                let phi = self.builder.build_phi(self.bool_type(), "iftmp")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                for (val, pred) in &incoming {
                    let bv = self.require_bool(*val)?;
                    phi.add_incoming(&[(&bv as &dyn BasicValue<'ctx>, *pred)]);
                }
                Ok(CgValue::Bool(phi.as_basic_value().into_int_value()))
            }
            CgValue::Number(_) => {
                let phi = self.builder.build_phi(self.f64_type(), "iftmp")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                for (val, pred) in &incoming {
                    let fv = self.require_number(*val)?;
                    phi.add_incoming(&[(&fv as &dyn BasicValue<'ctx>, *pred)]);
                }
                Ok(CgValue::Number(phi.as_basic_value().into_float_value()))
            }
            CgValue::Str(_) | CgValue::Object(_) | CgValue::Vector(_) | CgValue::Null => {
                let phi = self.builder.build_phi(self.ptr_type(), "iftmp")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                for (val, pred) in &incoming {
                    let ptr = match val {
                        CgValue::Str(p) | CgValue::Object(p) | CgValue::Vector(p) => *p,
                        CgValue::Null => self.ptr_type().const_null(),
                        _ => self.ptr_type().const_null(),
                    };
                    phi.add_incoming(&[(&ptr as &dyn BasicValue<'ctx>, *pred)]);
                }
                Ok(CgValue::Object(phi.as_basic_value().into_pointer_value()))
            }
            _ => Ok(CgValue::Void),
        }
    }

    fn lower_for(&mut self, fe: &ForExpr) -> CodegenResult<CgValue<'ctx>> {
        let fn_cur = self.current_fn()?;
        let i32_ty = self.context.i32_type();

        let iter_val = self.visit_expr(&fe.iterable)?;
        let iter_ty  = self.get_expr_type(&fe.iterable)?;
        let is_range = matches!(&iter_ty, HulkType::UserDefined(n) if n == "Range");
        let iter_ptr = match iter_val {
            CgValue::Object(p) | CgValue::Vector(p) => p,
            _ => return Err(CodegenError::Unsupported("for sobre tipo no iterable".into())),
        };

        let vec_count = if !is_range {
            let size_fn = self.require_fn("hulk_vec_size")?;
            let s_f64   = self.builder
                .build_call(size_fn, &[iter_ptr.into()], "fvsize")
                .map_err(|e| CodegenError::Builder(e.to_string()))?
                .try_as_basic_value().left().unwrap().into_float_value();
            Some(self.builder
                .build_float_to_signed_int(s_f64, i32_ty, "fvcount")
                .map_err(|e| CodegenError::Builder(e.to_string()))?)
        } else { None };

        // i32 alloca at entry block — evita conversiones f64↔i32 por iteración
        let idx_ptr = {
            let entry_bb = fn_cur.get_first_basic_block()
                .ok_or_else(|| CodegenError::Unsupported("for: función sin entry block".into()))?;
            let ab = self.context.create_builder();
            if let Some(first) = entry_bb.get_first_instruction() {
                ab.position_before(&first);
            } else {
                ab.position_at_end(entry_bb);
            }
            ab.build_alloca(i32_ty, "for_idx")
                .map_err(|e| CodegenError::Builder(e.to_string()))?
        };
        self.builder.build_store(idx_ptr, i32_ty.const_int(0, false))
            .map_err(|e| CodegenError::Builder(e.to_string()))?;

        let cond_bb = self.context.append_basic_block(fn_cur, "for_cond");
        let body_bb = self.context.append_basic_block(fn_cur, "for_body");
        let end_bb  = self.context.append_basic_block(fn_cur, "for_end");
        self.builder.build_unconditional_branch(cond_bb)
            .map_err(|e| CodegenError::Builder(e.to_string()))?;

        // for_cond
        self.builder.position_at_end(cond_bb);
        let cond = if is_range {
            let next_fn = self.require_fn("hulk_range_next")?;
            self.builder
                .build_call(next_fn, &[iter_ptr.into()], "rnext")
                .map_err(|e| CodegenError::Builder(e.to_string()))?
                .try_as_basic_value().left().unwrap().into_int_value()
        } else {
            let idx = self.builder
                .build_load(i32_ty, idx_ptr, "fidx")
                .map_err(|e| CodegenError::Builder(e.to_string()))?.into_int_value();
            self.builder
                .build_int_compare(IntPredicate::SLT, idx, vec_count.unwrap(), "flt")
                .map_err(|e| CodegenError::Builder(e.to_string()))?
        };
        self.builder.build_conditional_branch(cond, body_bb, end_bb)
            .map_err(|e| CodegenError::Builder(e.to_string()))?;

        // for_body
        self.builder.position_at_end(body_bb);
        self.push_scope();

        let elem_hulk_ty = match &iter_ty {
            HulkType::Vector(t) => *t.clone(),
            _ => HulkType::Number,  // Range → Number
        };

        let elem_val = if is_range {
            let curr_fn = self.require_fn("hulk_range_current")?;
            let num = self.builder
                .build_call(curr_fn, &[iter_ptr.into()], "rcurr")
                .map_err(|e| CodegenError::Builder(e.to_string()))?
                .try_as_basic_value().left().unwrap().into_float_value();
            CgValue::Number(num)
        } else {
            let idx = self.builder
                .build_load(i32_ty, idx_ptr, "fidx2")
                .map_err(|e| CodegenError::Builder(e.to_string()))?.into_int_value();
            let get_fn = self.require_fn("hulk_vec_get")?;
            let ep = self.builder
                .build_call(get_fn,
                    &[iter_ptr.into(), idx.into(),
                      i32_ty.const_int(ELEM_SIZE_BYTES, false).into()], "fep")
                .map_err(|e| CodegenError::Builder(e.to_string()))?
                .try_as_basic_value().left().unwrap().into_pointer_value();
            self.load_place(&Place { ptr: ep, hulk_ty: elem_hulk_ty.clone() }, &fe.var)?
        };

        let var_slot = self.create_entry_alloca_for(fn_cur, &fe.var, &elem_hulk_ty)?;
        self.store_place(&var_slot, elem_val)?;
        self.symbols.insert(fe.var.clone(), var_slot);

        self.visit_expr(&fe.body)?;

        if !is_range {
            let idx = self.builder
                .build_load(i32_ty, idx_ptr, "fidx3")
                .map_err(|e| CodegenError::Builder(e.to_string()))?.into_int_value();
            let next_idx = self.builder
                .build_int_add(idx, i32_ty.const_int(1, false), "fnext")
                .map_err(|e| CodegenError::Builder(e.to_string()))?;
            self.builder.build_store(idx_ptr, next_idx)
                .map_err(|e| CodegenError::Builder(e.to_string()))?;
        }

        self.pop_scope();
        if !self.is_current_block_terminated() {
            self.builder.build_unconditional_branch(cond_bb)
                .map_err(|e| CodegenError::Builder(e.to_string()))?;
        }

        self.builder.position_at_end(end_bb);
        Ok(CgValue::Void)
    }

    fn lower_method_call(
        &mut self,
        mc:   &MethodCallExpr,
        expr: &Expr,
    ) -> CodegenResult<CgValue<'ctx>> {
        let CgValue::Object(obj_ptr) = self.visit_expr(&mc.object)? else {
            unreachable!("MethodCall: receptor no es Object")
        };

        match self.get_expr_type(&mc.object)? {
            HulkType::UserDefined(type_name) => {
                let (fn_ptr, fn_type, sig) =
                    self.method_dispatch(obj_ptr, &type_name, &mc.method)?;

                let mut call_args: Vec<BasicMetadataValueEnum<'ctx>> = vec![obj_ptr.into()];
                for (i, arg_expr) in mc.args.iter().enumerate() {
                    let val      = self.visit_expr(arg_expr)?;
                    let expected = sig.params.get(i)
                        .map(|(_, t): &(String, HulkType)| t.clone())
                        .unwrap_or(HulkType::Object);
                    call_args.push(self.coerce_arg(val, &expected)?);
                }

                let result = self.builder
                    .build_indirect_call(fn_type, fn_ptr, &call_args, "method_result")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;

                Ok(self.call_result_to_cgvalue(result, &sig.return_type))
            }

            HulkType::Protocol(proto_name) => {
                let user_args: Vec<CgValue<'ctx>> = mc.args.iter()
                    .map(|a| self.visit_expr(a))
                    .collect::<CodegenResult<_>>()?;

                let return_ty = self.get_expr_type(expr)?;

                self.method_dispatch_protocol(
                    obj_ptr, &proto_name, &mc.method, &user_args, &return_ty,
                )
            }

            _ => unreachable!("MethodCall: tipo receptor inesperado"),
        }
    }

    fn lower_is(
        &mut self,
        inner:     &Expr,
        type_name: &TypeName,
    ) -> CodegenResult<CgValue<'ctx>> {
        let obj_val = self.visit_expr(inner)?;
        let obj_ptr = match obj_val {
            CgValue::Object(p) => p,
            _ => return Ok(CgValue::Bool(self.bool_type().const_int(0, false))),
        };

        let target_name = type_name.name().to_string();

        if target_name == "Object" {
            return Ok(CgValue::Bool(self.bool_type().const_int(1, false)));
        }

        let (min_tag, max_tag) = match self.type_registry.layouts.get(&target_name) {
            Some(layout) => (layout.type_tag, layout.max_tag),
            None => return Ok(CgValue::Bool(self.bool_type().const_int(0, false))),
        };

        let runtime_tag = self.builder
            .build_load(self.context.i32_type(), obj_ptr, "type_tag")
            .map_err(|e| CodegenError::Builder(e.to_string()))?
            .into_int_value();

        let i32_ty = self.context.i32_type();
        let ge = self.builder
            .build_int_compare(
                IntPredicate::UGE,
                runtime_tag,
                i32_ty.const_int(min_tag as u64, false),
                "tag_ge",
            )
            .map_err(|e| CodegenError::Builder(e.to_string()))?;
        let le = self.builder
            .build_int_compare(
                IntPredicate::ULE,
                runtime_tag,
                i32_ty.const_int(max_tag as u64, false),
                "tag_le",
            )
            .map_err(|e| CodegenError::Builder(e.to_string()))?;
        let result = self.builder
            .build_and(ge, le, "is_result")
            .map_err(|e| CodegenError::Builder(e.to_string()))?;

        Ok(CgValue::Bool(result))
    }

    fn lower_vector(&mut self, ve: &VectorExpr) -> CodegenResult<CgValue<'ctx>> {
        match ve {
            VectorExpr::Explicit { elements, .. } => {
                let n      = elements.len() as u64;
                let i32_ty = self.context.i32_type();
                let alloc  = self.require_fn("hulk_vec_alloc")?;
                let get    = self.require_fn("hulk_vec_get")?;

                let vec_ptr = self.builder
                    .build_call(alloc,
                        &[i32_ty.const_int(n, false).into(),
                          i32_ty.const_int(ELEM_SIZE_BYTES, false).into()], "vec")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .try_as_basic_value().left().unwrap()
                    .into_pointer_value();

                for (i, elem_expr) in elements.iter().enumerate() {
                    let val      = self.visit_expr(elem_expr)?;
                    let elem_ptr = self.builder
                        .build_call(get,
                            &[vec_ptr.into(),
                              i32_ty.const_int(i as u64, false).into(),
                              i32_ty.const_int(ELEM_SIZE_BYTES, false).into()], "ep")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?
                        .try_as_basic_value().left().unwrap()
                        .into_pointer_value();

                    match val {
                        CgValue::Number(f) => {
                            self.builder.build_store(elem_ptr, f)
                                .map_err(|e| CodegenError::Builder(e.to_string()))?;
                        }
                        CgValue::Bool(b) => {
                            let ext = self.builder
                                .build_int_z_extend(b, self.context.i64_type(), "b64")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?;
                            self.builder.build_store(elem_ptr, ext)
                                .map_err(|e| CodegenError::Builder(e.to_string()))?;
                        }
                        CgValue::Str(p) | CgValue::Object(p) | CgValue::Vector(p) => {
                            self.builder.build_store(elem_ptr, p)
                                .map_err(|e| CodegenError::Builder(e.to_string()))?;
                        }
                        CgValue::Null => {
                            self.builder.build_store(elem_ptr, self.ptr_type().const_null())
                                .map_err(|e| CodegenError::Builder(e.to_string()))?;
                        }
                        CgValue::Void => {}
                    }
                }

                Ok(CgValue::Vector(vec_ptr))
            }

            VectorExpr::Generator { body, var, iterable, .. } => {
                let fn_cur = self.current_fn()?;
                let i32_ty = self.context.i32_type();

                let iter_val = self.visit_expr(iterable)?;
                let iter_ty  = self.get_expr_type(iterable)?;
                let is_range = matches!(&iter_ty, HulkType::UserDefined(n) if n == "Range");
                let iter_ptr = match iter_val {
                    CgValue::Object(p) | CgValue::Vector(p) => p,
                    _ => return Err(CodegenError::Unsupported(
                        "generador sobre tipo no iterable".into())),
                };

                let count_val = if is_range {
                    let start = self.builder
                        .build_load(self.f64_type(), iter_ptr, "rstart")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?.into_float_value();
                    let end_gep = unsafe {
                        self.builder
                            .build_gep(self.context.i8_type(), iter_ptr,
                                &[i32_ty.const_int(8, false)], "end_ptr")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?
                    };
                    let end = self.builder
                        .build_load(self.f64_type(), end_gep, "rend")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?.into_float_value();
                    let diff = self.builder.build_float_sub(end, start, "rdiff")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?;
                    self.builder.build_float_to_signed_int(diff, i32_ty, "rcount")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?
                } else {
                    let size_fn  = self.require_fn("hulk_vec_size")?;
                    let size_f64 = self.builder
                        .build_call(size_fn, &[iter_ptr.into()], "vsize")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?
                        .try_as_basic_value().left().unwrap().into_float_value();
                    self.builder.build_float_to_signed_int(size_f64, i32_ty, "vcount")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?
                };

                let alloc_fn   = self.require_fn("hulk_vec_alloc")?;
                let result_ptr = self.builder
                    .build_call(alloc_fn,
                        &[count_val.into(), i32_ty.const_int(ELEM_SIZE_BYTES, false).into()], "gen_vec")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .try_as_basic_value().left().unwrap().into_pointer_value();

                // i32 alloca at entry block para el índice del generador
                let idx_ptr = {
                    let entry_bb = fn_cur.get_first_basic_block()
                        .ok_or_else(|| CodegenError::Unsupported("gen: función sin entry block".into()))?;
                    let ab = self.context.create_builder();
                    if let Some(first) = entry_bb.get_first_instruction() {
                        ab.position_before(&first);
                    } else {
                        ab.position_at_end(entry_bb);
                    }
                    ab.build_alloca(i32_ty, "gen_idx")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?
                };
                self.builder.build_store(idx_ptr, i32_ty.const_int(0, false))
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;

                let cond_bb = self.context.append_basic_block(fn_cur, "gen_cond");
                let body_bb = self.context.append_basic_block(fn_cur, "gen_body");
                let end_bb  = self.context.append_basic_block(fn_cur, "gen_end");
                self.builder.build_unconditional_branch(cond_bb)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;

                // gen_cond: idx < count
                self.builder.position_at_end(cond_bb);
                let idx = self.builder
                    .build_load(i32_ty, idx_ptr, "idx_i32")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?.into_int_value();
                let cond = self.builder
                    .build_int_compare(IntPredicate::SLT, idx, count_val, "gen_lt")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                self.builder.build_conditional_branch(cond, body_bb, end_bb)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;

                // gen_body
                self.builder.position_at_end(body_bb);
                self.push_scope();

                let elem_hulk_ty = match &iter_ty {
                    HulkType::Vector(t) => *t.clone(),
                    _ => HulkType::Number,  // Range → Number
                };

                let elem_val = if is_range {
                    let next_fn = self.require_fn("hulk_range_next")?;
                    self.builder.build_call(next_fn, &[iter_ptr.into()], "")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?;
                    let curr_fn = self.require_fn("hulk_range_current")?;
                    let num = self.builder
                        .build_call(curr_fn, &[iter_ptr.into()], "rcurr")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?
                        .try_as_basic_value().left().unwrap().into_float_value();
                    CgValue::Number(num)
                } else {
                    let get_fn = self.require_fn("hulk_vec_get")?;
                    let ep = self.builder
                        .build_call(get_fn,
                            &[iter_ptr.into(), idx.into(),
                              i32_ty.const_int(ELEM_SIZE_BYTES, false).into()], "ep")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?
                        .try_as_basic_value().left().unwrap().into_pointer_value();
                    self.load_place(&Place { ptr: ep, hulk_ty: elem_hulk_ty.clone() }, var)?
                };

                let var_slot = self.create_entry_alloca_for(fn_cur, var, &elem_hulk_ty)?;
                self.store_place(&var_slot, elem_val)?;
                self.symbols.insert(var.clone(), var_slot);

                let body_val = self.visit_expr(body)?;
                let get_fn   = self.require_fn("hulk_vec_get")?;
                let dest_ptr = self.builder
                    .build_call(get_fn,
                        &[result_ptr.into(), idx.into(),
                          i32_ty.const_int(ELEM_SIZE_BYTES, false).into()], "dp")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .try_as_basic_value().left().unwrap().into_pointer_value();
                match body_val {
                    CgValue::Number(f) => {
                        self.builder.build_store(dest_ptr, f)
                            .map_err(|e| CodegenError::Builder(e.to_string()))?;
                    }
                    CgValue::Bool(b) => {
                        let ext = self.builder
                            .build_int_z_extend(b, self.context.i64_type(), "b64")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?;
                        self.builder.build_store(dest_ptr, ext)
                            .map_err(|e| CodegenError::Builder(e.to_string()))?;
                    }
                    CgValue::Str(p) | CgValue::Object(p) | CgValue::Vector(p) => {
                        self.builder.build_store(dest_ptr, p)
                            .map_err(|e| CodegenError::Builder(e.to_string()))?;
                    }
                    _ => {}
                }

                let next_idx = self.builder
                    .build_int_add(idx, i32_ty.const_int(1, false), "next_idx")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                self.builder.build_store(idx_ptr, next_idx)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;

                self.pop_scope();
                if !self.is_current_block_terminated() {
                    self.builder.build_unconditional_branch(cond_bb)
                        .map_err(|e| CodegenError::Builder(e.to_string()))?;
                }

                self.builder.position_at_end(end_bb);
                Ok(CgValue::Vector(result_ptr))
            }
        }
    }
}
