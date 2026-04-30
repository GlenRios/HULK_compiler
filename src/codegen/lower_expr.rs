use inkwell::FloatPredicate;
use inkwell::IntPredicate;
use inkwell::module::Linkage;
use inkwell::types::BasicType;
use inkwell::values::{BasicMetadataValueEnum, BasicValue};

use crate::parser::ast::{
    AssignOp, BinaryOp, Expr, ExprKind, Literal, PostfixOp, UnaryOp,
};
use crate::semantic::HulkType;

use super::context::CodegenContext;
use super::error::{CodegenError, CodegenResult};
use super::symbols::Place;
use super::value::CgValue;
use super::visitor::ExprVisitor;

impl<'ctx> CodegenContext<'ctx> {
    fn eval_number(&mut self, expr: &Expr) -> CodegenResult<inkwell::values::FloatValue<'ctx>> {
        let value = self.visit_expr(expr)?;
        self.require_number(value)
    }

    fn eval_bool(&mut self, expr: &Expr) -> CodegenResult<inkwell::values::IntValue<'ctx>> {
        let value = self.visit_expr(expr)?;
        self.require_bool(value)
    }

    /// Devuelve el Place de un lvalue variable (Identifier).
    fn eval_lvalue_slot<'a>(&'a self, expr: &Expr) -> CodegenResult<&'a Place<'ctx>> {
        match &expr.kind {
            ExprKind::Identifier { name } => self
                .symbols
                .get(name)
                .ok_or_else(|| CodegenError::UnknownVariable(name.clone())),
            _ => Err(CodegenError::InvalidLValue),
        }
    }

    /// Devuelve el HulkType de una expresión consultando el side table del TypeChecker.
    /// Reemplaza infer_expr_type — siempre correcto, nunca Unknown para expr bien tipadas.
    fn get_expr_type(&self, expr: &Expr) -> HulkType {
        self.expr_types.get(&expr.id)
            .cloned()
            .expect("expression should be annotated by TypeChecker")
    }
}

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

            // ── Binarias ──────────────────────────────────────────────────────
            ExprKind::Binary(bin) => {
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
                    BinaryOp::Eq => {
                        let l = self.eval_number(&bin.left)?;
                        let r = self.eval_number(&bin.right)?;
                        Ok(CgValue::Bool(
                            self.builder.build_float_compare(FloatPredicate::OEQ, l, r, "eqtmp")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
                    }
                    BinaryOp::NotEq => {
                        let l = self.eval_number(&bin.left)?;
                        let r = self.eval_number(&bin.right)?;
                        Ok(CgValue::Bool(
                            self.builder.build_float_compare(FloatPredicate::ONE, l, r, "neqtmp")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
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
                        let l = self.eval_bool(&bin.left)?;
                        let r = self.eval_bool(&bin.right)?;
                        Ok(CgValue::Bool(
                            self.builder.build_and(l, r, "andtmp")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
                    }
                    BinaryOp::Or => {
                        let l = self.eval_bool(&bin.left)?;
                        let r = self.eval_bool(&bin.right)?;
                        Ok(CgValue::Bool(
                            self.builder.build_or(l, r, "ortmp")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
                    }
                    BinaryOp::Concat => {
                        let l = self.visit_expr(&bin.left)?;
                        let r = self.visit_expr(&bin.right)?;
                        let lp = self.cgvalue_to_str(l)?;
                        let rp = self.cgvalue_to_str(r)?;
                        let f  = self.require_fn("hulk_str_concat");
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
                        let f  = self.require_fn("hulk_str_concat_space");
                        let ptr = self.builder
                            .build_call(f, &[lp.into(), rp.into()], "dconcat")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?
                            .try_as_basic_value().left().unwrap().into_pointer_value();
                        Ok(CgValue::Str(ptr))
                    }
                }
            }

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

            // ── Postfix (++/--) — solo Number ─────────────────────────────────
            ExprKind::Postfix(postfix) => {
                let slot = self.eval_lvalue_slot(&postfix.operand)?.clone();
                // El typechecker garantiza que postfix solo aplica a Number
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

            // ── Asignación ────────────────────────────────────────────────────
            ExprKind::Assign(assign) => {
                // Resolver el lvalue: variable local o campo de objeto
                let place = match &assign.target.kind {
                    ExprKind::Identifier { .. } => {
                        self.eval_lvalue_slot(&assign.target)?.clone()
                    }
                    ExprKind::Access(ae) => {
                        let CgValue::Object(obj_ptr) = self.visit_expr(&ae.object)? else {
                            unreachable!("lvalue Access: receptor no es Object")
                        };
                        let HulkType::UserDefined(type_name) = self.get_expr_type(&ae.object) else {
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
                    // Compound assigns son solo para Number (typechecker lo garantiza)
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

            // ── Llamada a función ─────────────────────────────────────────────
            ExprKind::Call(call) => {
                // ── base(args) — llamada directa al método del padre ──────────
                // Según la spec: base() dentro de Knight.name() llama a Person.name()
                // Es una llamada ESTÁTICA (no vtable) — la función es conocida en compile time
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

                    // Ancestro que implementa el método (puede ser abuelo, bisabuelo...)
                    let impl_type = self.type_hierarchy
                        .find_method_impl_type(&parent_name, &method_name)
                        .unwrap_or(parent_name);

                    let sig = self.type_hierarchy.types
                        .get(&impl_type)
                        .and_then(|ti| ti.methods.get(&method_name))
                        .cloned()
                        .ok_or_else(|| CodegenError::Unsupported(
                            format!("método '{}' no encontrado en '{}'", method_name, impl_type)))?;

                    // self_ptr siempre es el primer argumento del método
                    let self_ptr = self.self_ptr
                        .ok_or_else(|| CodegenError::Unsupported(
                            "base() sin self_ptr".into()))?;
                    let mut call_args: Vec<BasicMetadataValueEnum<'ctx>> = vec![self_ptr.into()];
                    for (i, arg_expr) in call.args.iter().enumerate() {
                        let val      = self.visit_expr(arg_expr)?;
                        let expected = sig.params.get(i)
                            .map(|(_, t)| t.clone())
                            .unwrap_or(HulkType::Object);
                        call_args.push(self.coerce_arg(val, &expected)?);
                    }

                    // Llamada directa por nombre — no carga ningún puntero de vtable
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

                // ── llamada normal por nombre ─────────────────────────────────
                let callee_name = match &call.callee.kind {
                    ExprKind::Identifier { name } => name.clone(),
                    _ => return Err(CodegenError::Unsupported(
                        "solo se soporta llamada directa por nombre".to_string(),
                    )),
                };

                // ── Builtins especiales ───────────────────────────────────────
                match callee_name.as_str() {
                    // print(x) — convierte el arg a string y lo imprime
                    "print" => {
                        let arg_val = self.visit_expr(&call.args[0])?;
                        let str_ptr = self.cgvalue_to_str(arg_val)?;
                        let print_fn = self.require_fn("hulk_print");
                        self.builder
                            .build_call(print_fn, &[str_ptr.into()], "")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?;
                        return Ok(CgValue::Null);
                    }

                    // rand() → Number en [0,1]
                    "rand" => {
                        let f = self.require_fn("hulk_rand");
                        let v = self.builder
                            .build_call(f, &[], "randtmp")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?
                            .try_as_basic_value().left().unwrap().into_float_value();
                        return Ok(CgValue::Number(v));
                    }

                    // sqrt / sin / cos / exp — una sola arg f64 → f64
                    "sqrt" | "sin" | "cos" | "exp" => {
                        let arg = self.eval_number(&call.args[0])?;
                        let f = self.require_fn(&callee_name);
                        let v = self.builder
                            .build_call(f, &[arg.into()], "mathtmp")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?
                            .try_as_basic_value().left().unwrap().into_float_value();
                        return Ok(CgValue::Number(v));
                    }

                    // log(base, value) en HULK = ln(value) / ln(base)
                    "log" => {
                        let base = self.eval_number(&call.args[0])?;
                        let val  = self.eval_number(&call.args[1])?;
                        let ln_fn = self.require_fn("log");
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

                    _ => {}
                }

                // ── Llamada general ───────────────────────────────────────────
                let function = self.functions.get(&callee_name).copied()
                    .or_else(|| self.module.get_function(&callee_name))
                    .ok_or_else(|| CodegenError::UnknownFunction(callee_name.clone()))?;

                let mut args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(call.args.len());
                for arg in &call.args {
                    let v = self.eval_number(arg)?;
                    args.push(v.into());
                }
                let call_site = self.builder
                    .build_call(function, &args, "calltmp")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                match call_site.try_as_basic_value().left() {
                    Some(v) => Ok(CgValue::Number(v.into_float_value())),
                    None    => Ok(CgValue::Void),
                }
            }

            // ── Bloque ────────────────────────────────────────────────────────
            ExprKind::Block(block) => {
                let mut last = CgValue::Void;
                for e in &block.body {
                    if self.is_current_block_terminated() { break; }
                    last = self.visit_expr(e)?;
                }
                Ok(last)
            }

            // ── Let ───────────────────────────────────────────────────────────
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

            // ── If / elif / else — PHI tipado ─────────────────────────────────
            ExprKind::If(if_expr) => {
                let function = self.current_fn()?;
                let merge_block = self.context.append_basic_block(function, "if_merge");

                // Acumula (CgValue, BasicBlock) de cada rama
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
                    }
                    incoming.push((val, end));
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
                    }
                    incoming.push((val, end));
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
                    }
                    incoming.push((val, end));
                }

                // PHI tipado según el tipo de la primera rama
                self.builder.position_at_end(merge_block);
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
                    _ => {
                        // Number u otros — coerce a f64
                        let phi = self.builder.build_phi(self.f64_type(), "iftmp")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?;
                        for (val, pred) in &incoming {
                            let fv = self.require_number(*val)?;
                            phi.add_incoming(&[(&fv as &dyn BasicValue<'ctx>, *pred)]);
                        }
                        Ok(CgValue::Number(phi.as_basic_value().into_float_value()))
                    }
                }
            }

            // ── While ─────────────────────────────────────────────────────────
            ExprKind::While(while_expr) => {
                let function = self.current_fn()?;
                let cond_block = self.context.append_basic_block(function, "while_cond");
                let body_block = self.context.append_basic_block(function, "while_body");
                let end_block  = self.context.append_basic_block(function, "while_end");

                self.builder.build_unconditional_branch(cond_block)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;

                self.builder.position_at_end(cond_block);
                let cond = self.eval_bool(&while_expr.condition)?;
                self.builder.build_conditional_branch(cond, body_block, end_block)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;

                self.builder.position_at_end(body_block);
                let _ = self.visit_expr(&while_expr.body)?;
                if !self.is_current_block_terminated() {
                    self.builder.build_unconditional_branch(cond_block)
                        .map_err(|e| CodegenError::Builder(e.to_string()))?;
                }

                self.builder.position_at_end(end_block);
                Ok(CgValue::Void)
            }

            // ── Stubs ─────────────────────────────────────────────────────────
            ExprKind::For(_)        => Err(CodegenError::Unsupported("for aun no implementado".to_string())),
            ExprKind::New(new_expr) => {
                let type_name = new_expr.type_name.name().to_string();
                let ctor_name = format!("__hulk_ctor_{}", type_name);

                let ctor_fn = self.module.get_function(&ctor_name)
                    .ok_or_else(|| CodegenError::UnknownFunction(ctor_name.clone()))?;

                // Tipos esperados por el constructor (del TypeHierarchy)
                let ctor_param_types: Vec<HulkType> = self.type_hierarchy.types
                    .get(&type_name)
                    .map(|ti| ti.constructor_params.iter().map(|(_, t)| t.clone()).collect())
                    .unwrap_or_default();

                // Evaluar cada argumento y coercionar al tipo LLVM correcto
                let mut args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = vec![];
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
                // 1. evaluar el objeto → puntero al struct en heap
                let CgValue::Object(obj_ptr) = self.visit_expr(&ae.object)? else {
                    unreachable!("objeto UserDefined produjo un CgValue que no es Object")
                };

                // 2. tipo del objeto — el TypeChecker garantizó que es UserDefined
                let HulkType::UserDefined(type_name) = self.get_expr_type(&ae.object) else {
                    unreachable!("access sobre tipo no UserDefined: el TypeChecker debería haber rechazado esto")
                };

                // 3. calcular dirección del campo y leerlo
                let place = self.field_place(obj_ptr, &type_name, &ae.field)?;
                self.load_place(&place, &ae.field)
            }
            ExprKind::MethodCall(mc) => {
                // 1. evaluar objeto → puntero al struct en heap
                let CgValue::Object(obj_ptr) = self.visit_expr(&mc.object)? else {
                    unreachable!("MethodCall: receptor no es Object")
                };

                // 2. tipo estático del objeto — garantizado por el TypeChecker
                let HulkType::UserDefined(type_name) = self.get_expr_type(&mc.object) else {
                    unreachable!("MethodCall: tipo no es UserDefined")
                };

                // 3. dispatch: carga vtable_ptr → slot → fn_ptr, firma LLVM y semántica
                let (fn_ptr, fn_type, sig) =
                    self.method_dispatch(obj_ptr, &type_name, &mc.method)?;

                // 4. obj_ptr es el primer argumento (self del método)
                let mut call_args: Vec<BasicMetadataValueEnum<'ctx>> =
                    vec![obj_ptr.into()];
                for (i, arg_expr) in mc.args.iter().enumerate() {
                    let val      = self.visit_expr(arg_expr)?;
                    let expected = sig.params.get(i)
                        .map(|(_, t)| t.clone())
                        .unwrap_or(HulkType::Object);
                    call_args.push(self.coerce_arg(val, &expected)?);
                }

                // 5. llamada indirecta vía vtable (dispatch dinámico)
                let result = self.builder
                    .build_indirect_call(fn_type, fn_ptr, &call_args, "method_result")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;

                // 6. convertir resultado a CgValue según tipo de retorno
                match &sig.return_type {
                    HulkType::Number  => Ok(CgValue::Number(
                        result.try_as_basic_value().left()
                            .ok_or_else(|| CodegenError::Unsupported(
                                "método sin retorno usado como valor".into()))?
                            .into_float_value()
                    )),
                    HulkType::Boolean => Ok(CgValue::Bool(
                        result.try_as_basic_value().left()
                            .ok_or_else(|| CodegenError::Unsupported(
                                "método sin retorno usado como valor".into()))?
                            .into_int_value()
                    )),
                    HulkType::StringT => Ok(CgValue::Str(
                        result.try_as_basic_value().left()
                            .ok_or_else(|| CodegenError::Unsupported(
                                "método sin retorno usado como valor".into()))?
                            .into_pointer_value()
                    )),
                    HulkType::Null | HulkType::Unknown => Ok(CgValue::Null),
                    _ => Ok(CgValue::Object(
                        result.try_as_basic_value().left()
                            .ok_or_else(|| CodegenError::Unsupported(
                                "método sin retorno usado como valor".into()))?
                            .into_pointer_value()
                    )),
                }
            }
            ExprKind::Index(_)      => Err(CodegenError::Unsupported("index aun no implementado".to_string())),
            ExprKind::Is { expr: inner, type_name } => {
                // 1. evaluar el objeto
                let obj_val = self.visit_expr(inner)?;
                let obj_ptr = match obj_val {
                    CgValue::Object(p) => p,
                    // null y no-objetos nunca conforman con ningún tipo
                    _ => return Ok(CgValue::Bool(self.bool_type().const_int(0, false))),
                };

                let target_name = type_name.name().to_string();

                // 2. caso especial: is Object — todo objeto lo es siempre
                if target_name == "Object" {
                    return Ok(CgValue::Bool(self.bool_type().const_int(1, false)));
                }

                // 3. obtener el rango DFS del tipo objetivo
                let (min_tag, max_tag) = match self.type_registry.layouts.get(&target_name) {
                    Some(layout) => (layout.type_tag, layout.max_tag),
                    None => return Ok(CgValue::Bool(self.bool_type().const_int(0, false))),
                };

                // 4. cargar el type_tag del objeto desde el campo 0 (offset 0, i32)
                let runtime_tag = self.builder
                    .build_load(self.context.i32_type(), obj_ptr, "type_tag")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .into_int_value();

                // 5. range check: min_tag <= runtime_tag <= max_tag
                //    Equivale a comprobar si el tipo real es el target o algún subtipo
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
            ExprKind::As { expr: inner, .. } => {
                // As es un no-op en LLVM con opaque pointers.
                // El TypeChecker ya validó el cast. El valor en memoria no cambia.
                self.visit_expr(inner)
            }
            ExprKind::Base       => Err(CodegenError::Unsupported("base aun no implementado".to_string())),
            ExprKind::Vector(_)     => Err(CodegenError::Unsupported("vector aun no implementado".to_string())),
        }
    }
}
