use inkwell::FloatPredicate;
use inkwell::IntPredicate;
use inkwell::module::Linkage;
use inkwell::values::{BasicMetadataValueEnum, BasicValue};

use crate::parser::ast::{
    AssignOp, BinaryOp, Expr, Literal, PostfixOp, UnaryOp,
};
use crate::semantic::HulkType;

use super::context::CodegenContext;
use super::error::{CodegenError, CodegenResult};
use super::symbols::VarSlot;
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

    /// Devuelve el VarSlot de un lvalue válido (Identifier por ahora).
    fn eval_lvalue_slot<'a>(&'a self, expr: &Expr) -> CodegenResult<&'a VarSlot<'ctx>> {
        match expr {
            Expr::Identifier { name, .. } => self
                .symbols
                .get(name)
                .ok_or_else(|| CodegenError::UnknownVariable(name.clone())),
            _ => Err(CodegenError::InvalidLValue),
        }
    }
}

impl<'ctx> ExprVisitor<'ctx> for CodegenContext<'ctx> {
    fn visit_expr(&mut self, expr: &Expr) -> CodegenResult<CgValue<'ctx>> {
        match expr {
            // ── Literales ─────────────────────────────────────────────────────
            Expr::Literal(lit) => match lit {
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
            Expr::Identifier { name, .. } => {
                let slot = self
                    .symbols
                    .get(name)
                    .ok_or_else(|| CodegenError::UnknownVariable(name.clone()))?
                    .clone();
                self.load_slot(&slot, &format!("load_{name}"))
            }

            // ── Binarias ──────────────────────────────────────────────────────
            Expr::Binary(bin) => {
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
            Expr::Unary(unary) => match unary.op {
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
            Expr::Postfix(postfix) => {
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
            Expr::Assign(assign) => {
                let slot = self.eval_lvalue_slot(&assign.target)?.clone();
                let rhs = self.visit_expr(&assign.value)?;

                match assign.op {
                    AssignOp::Assign => {
                        self.store_slot(&slot, rhs)?;
                        Ok(rhs)
                    }
                    // Compound assigns son solo para Number (typechecker lo garantiza)
                    AssignOp::PlusAssign => {
                        let old = self.builder
                            .build_load(self.f64_type(), slot.ptr, "old")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?
                            .into_float_value();
                        let rhs_f = self.require_number(rhs)?;
                        let result = self.builder.build_float_add(old, rhs_f, "plus_assign")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?;
                        self.builder.build_store(slot.ptr, result)
                            .map_err(|e| CodegenError::Builder(e.to_string()))?;
                        Ok(CgValue::Number(result))
                    }
                    AssignOp::MinusAssign => {
                        let old = self.builder
                            .build_load(self.f64_type(), slot.ptr, "old")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?
                            .into_float_value();
                        let rhs_f = self.require_number(rhs)?;
                        let result = self.builder.build_float_sub(old, rhs_f, "minus_assign")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?;
                        self.builder.build_store(slot.ptr, result)
                            .map_err(|e| CodegenError::Builder(e.to_string()))?;
                        Ok(CgValue::Number(result))
                    }
                    AssignOp::MulAssign => {
                        let old = self.builder
                            .build_load(self.f64_type(), slot.ptr, "old")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?
                            .into_float_value();
                        let rhs_f = self.require_number(rhs)?;
                        let result = self.builder.build_float_mul(old, rhs_f, "mul_assign")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?;
                        self.builder.build_store(slot.ptr, result)
                            .map_err(|e| CodegenError::Builder(e.to_string()))?;
                        Ok(CgValue::Number(result))
                    }
                    AssignOp::DivAssign => {
                        let old = self.builder
                            .build_load(self.f64_type(), slot.ptr, "old")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?
                            .into_float_value();
                        let rhs_f = self.require_number(rhs)?;
                        let result = self.builder.build_float_div(old, rhs_f, "div_assign")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?;
                        self.builder.build_store(slot.ptr, result)
                            .map_err(|e| CodegenError::Builder(e.to_string()))?;
                        Ok(CgValue::Number(result))
                    }
                    AssignOp::ModAssign => {
                        let old = self.builder
                            .build_load(self.f64_type(), slot.ptr, "old")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?
                            .into_float_value();
                        let rhs_f = self.require_number(rhs)?;
                        let result = self.builder.build_float_rem(old, rhs_f, "mod_assign")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?;
                        self.builder.build_store(slot.ptr, result)
                            .map_err(|e| CodegenError::Builder(e.to_string()))?;
                        Ok(CgValue::Number(result))
                    }
                }
            }

            // ── Llamada a función ─────────────────────────────────────────────
            Expr::Call(call) => {
                let callee_name = match &*call.callee {
                    Expr::Identifier { name, .. } => name.clone(),
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
            Expr::Block(block) => {
                let mut last = CgValue::Void;
                for e in &block.body {
                    if self.is_current_block_terminated() { break; }
                    last = self.visit_expr(e)?;
                }
                Ok(last)
            }

            // ── Let ───────────────────────────────────────────────────────────
            Expr::Let(let_expr) => {
                let function = self.current_fn()?;
                self.push_scope();

                for binding in &let_expr.bindings {
                    let val  = self.visit_expr(&binding.value)?;
                    let ty   = val.hulk_type();
                    let slot = self.create_entry_alloca_for(function, &binding.name, &ty)?;
                    self.store_slot(&slot, val)?;
                    self.symbols.insert(binding.name.clone(), slot);
                }

                let out = self.visit_expr(&let_expr.body)?;
                self.pop_scope();
                Ok(out)
            }

            // ── If / elif / else — PHI tipado ─────────────────────────────────
            Expr::If(if_expr) => {
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
            Expr::While(while_expr) => {
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
            Expr::For(_)        => Err(CodegenError::Unsupported("for aun no implementado".to_string())),
            Expr::New(_)        => Err(CodegenError::Unsupported("new aun no implementado".to_string())),
            Expr::Access(_)     => Err(CodegenError::Unsupported("access aun no implementado".to_string())),
            Expr::MethodCall(_) => Err(CodegenError::Unsupported("method_call aun no implementado".to_string())),
            Expr::Index(_)      => Err(CodegenError::Unsupported("index aun no implementado".to_string())),
            Expr::Is { .. }     => Err(CodegenError::Unsupported("is aun no implementado".to_string())),
            Expr::As { .. }     => Err(CodegenError::Unsupported("as aun no implementado".to_string())),
            Expr::Base(_)       => Err(CodegenError::Unsupported("base aun no implementado".to_string())),
            Expr::Vector(_)     => Err(CodegenError::Unsupported("vector aun no implementado".to_string())),
        }
    }
}
