use inkwell::FloatPredicate;
use inkwell::IntPredicate;
use inkwell::values::BasicMetadataValueEnum;

use crate::parser::ast::{
    AssignOp, BinaryOp, Expr, Literal, PostfixOp, UnaryOp,
};

use super::context::CodegenContext;
use super::error::{CodegenError, CodegenResult};
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

    fn eval_lvalue_ptr(&self, expr: &Expr) -> CodegenResult<inkwell::values::PointerValue<'ctx>> {
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
                Literal::Null { .. } => Ok(CgValue::Number(self.f64_type().const_float(0.0))),
                Literal::String { .. } => Err(CodegenError::Unsupported(
                    "literal String aun no implementado".to_string(),
                )),
                Literal::Char { .. } => Err(CodegenError::Unsupported(
                    "literal Char aun no implementado".to_string(),
                )),
            },

            Expr::Identifier { name, .. } => {
                let ptr = self
                    .symbols
                    .get(name)
                    .ok_or_else(|| CodegenError::UnknownVariable(name.clone()))?;
                let loaded = self
                    .builder
                    .build_load(self.f64_type(), ptr, &format!("load_{name}"))
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .into_float_value();
                Ok(CgValue::Number(loaded))
            }

            Expr::Binary(bin) => {
                let op = &bin.op;
                match op {
                    BinaryOp::Add => {
                        let l = self.eval_number(&bin.left)?;
                        let r = self.eval_number(&bin.right)?;
                        Ok(CgValue::Number(
                            self.builder
                                .build_float_add(l, r, "addtmp")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
                    }
                    BinaryOp::Sub => {
                        let l = self.eval_number(&bin.left)?;
                        let r = self.eval_number(&bin.right)?;
                        Ok(CgValue::Number(
                            self.builder
                                .build_float_sub(l, r, "subtmp")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
                    }
                    BinaryOp::Mul => {
                        let l = self.eval_number(&bin.left)?;
                        let r = self.eval_number(&bin.right)?;
                        Ok(CgValue::Number(
                            self.builder
                                .build_float_mul(l, r, "multmp")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
                    }
                    BinaryOp::Div => {
                        let l = self.eval_number(&bin.left)?;
                        let r = self.eval_number(&bin.right)?;
                        Ok(CgValue::Number(
                            self.builder
                                .build_float_div(l, r, "divtmp")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
                    }
                    BinaryOp::Mod => {
                        let l = self.eval_number(&bin.left)?;
                        let r = self.eval_number(&bin.right)?;
                        Ok(CgValue::Number(
                            self.builder
                                .build_float_rem(l, r, "modtmp")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
                    }
                    BinaryOp::Eq => {
                        let l = self.eval_number(&bin.left)?;
                        let r = self.eval_number(&bin.right)?;
                        Ok(CgValue::Bool(
                            self.builder
                                .build_float_compare(FloatPredicate::OEQ, l, r, "eqtmp")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
                    }
                    BinaryOp::NotEq => {
                        let l = self.eval_number(&bin.left)?;
                        let r = self.eval_number(&bin.right)?;
                        Ok(CgValue::Bool(
                            self.builder
                                .build_float_compare(FloatPredicate::ONE, l, r, "neqtmp")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
                    }
                    BinaryOp::Less => {
                        let l = self.eval_number(&bin.left)?;
                        let r = self.eval_number(&bin.right)?;
                        Ok(CgValue::Bool(
                            self.builder
                                .build_float_compare(FloatPredicate::OLT, l, r, "lttmp")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
                    }
                    BinaryOp::Greater => {
                        let l = self.eval_number(&bin.left)?;
                        let r = self.eval_number(&bin.right)?;
                        Ok(CgValue::Bool(
                            self.builder
                                .build_float_compare(FloatPredicate::OGT, l, r, "gttmp")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
                    }
                    BinaryOp::LessEq => {
                        let l = self.eval_number(&bin.left)?;
                        let r = self.eval_number(&bin.right)?;
                        Ok(CgValue::Bool(
                            self.builder
                                .build_float_compare(FloatPredicate::OLE, l, r, "letmp")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
                    }
                    BinaryOp::GreaterEq => {
                        let l = self.eval_number(&bin.left)?;
                        let r = self.eval_number(&bin.right)?;
                        Ok(CgValue::Bool(
                            self.builder
                                .build_float_compare(FloatPredicate::OGE, l, r, "getmp")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
                    }
                    BinaryOp::And => {
                        let l = self.eval_bool(&bin.left)?;
                        let r = self.eval_bool(&bin.right)?;
                        Ok(CgValue::Bool(
                            self.builder
                                .build_and(l, r, "andtmp")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
                    }
                    BinaryOp::Or => {
                        let l = self.eval_bool(&bin.left)?;
                        let r = self.eval_bool(&bin.right)?;
                        Ok(CgValue::Bool(
                            self.builder
                                .build_or(l, r, "ortmp")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
                    }
                    BinaryOp::Power => Err(CodegenError::Unsupported(
                        "operador Power aun no implementado".to_string(),
                    )),
                    BinaryOp::Concat | BinaryOp::DoubleConcat => Err(CodegenError::Unsupported(
                        "concatenacion de strings aun no implementada".to_string(),
                    )),
                }
            }

            Expr::Unary(unary) => match unary.op {
                UnaryOp::Neg => {
                    let value = self.eval_number(&unary.operand)?;
                    let neg = self
                        .builder
                        .build_float_neg(value, "negtmp")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?;
                    Ok(CgValue::Number(neg))
                }
                UnaryOp::Not => {
                    let value = self.eval_bool(&unary.operand)?;
                    let not = self
                        .builder
                        .build_not(value, "nottmp")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?;
                    Ok(CgValue::Bool(not))
                }
            },

            Expr::Postfix(postfix) => {
                let ptr = self.eval_lvalue_ptr(&postfix.operand)?;
                let old = self
                    .builder
                    .build_load(self.f64_type(), ptr, "post_load")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .into_float_value();

                let delta = self.f64_type().const_float(1.0);
                let new_value = match postfix.op {
                    PostfixOp::Increment => self
                        .builder
                        .build_float_add(old, delta, "post_inc")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?,
                    PostfixOp::Decrement => self
                        .builder
                        .build_float_sub(old, delta, "post_dec")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?,
                };

                self.builder
                    .build_store(ptr, new_value)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                Ok(CgValue::Number(old))
            }

            Expr::Assign(assign) => {
                let ptr = self.eval_lvalue_ptr(&assign.target)?;
                let rhs = self.eval_number(&assign.value)?;

                let result = match assign.op {
                    AssignOp::Assign => rhs,
                    AssignOp::PlusAssign => {
                        let old = self
                            .builder
                            .build_load(self.f64_type(), ptr, "old_plus")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?
                            .into_float_value();
                        self.builder
                            .build_float_add(old, rhs, "plus_assign")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?
                    }
                    AssignOp::MinusAssign => {
                        let old = self
                            .builder
                            .build_load(self.f64_type(), ptr, "old_minus")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?
                            .into_float_value();
                        self.builder
                            .build_float_sub(old, rhs, "minus_assign")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?
                    }
                    AssignOp::MulAssign => {
                        let old = self
                            .builder
                            .build_load(self.f64_type(), ptr, "old_mul")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?
                            .into_float_value();
                        self.builder
                            .build_float_mul(old, rhs, "mul_assign")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?
                    }
                    AssignOp::DivAssign => {
                        let old = self
                            .builder
                            .build_load(self.f64_type(), ptr, "old_div")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?
                            .into_float_value();
                        self.builder
                            .build_float_div(old, rhs, "div_assign")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?
                    }
                    AssignOp::ModAssign => {
                        let old = self
                            .builder
                            .build_load(self.f64_type(), ptr, "old_mod")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?
                            .into_float_value();
                        self.builder
                            .build_float_rem(old, rhs, "mod_assign")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?
                    }
                };

                self.builder
                    .build_store(ptr, result)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                Ok(CgValue::Number(result))
            }

            Expr::Call(call) => {
                let callee_name = match &*call.callee {
                    Expr::Identifier { name, .. } => name.clone(),
                    _ => {
                        return Err(CodegenError::Unsupported(
                            "solo se soporta llamada directa por nombre".to_string(),
                        ))
                    }
                };

                let function = self
                    .functions
                    .get(&callee_name)
                    .copied()
                    .or_else(|| self.module.get_function(&callee_name))
                    .ok_or_else(|| CodegenError::UnknownFunction(callee_name.clone()))?;

                let mut arg_values: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(call.args.len());
                for arg in &call.args {
                    let num = self.eval_number(arg)?;
                    arg_values.push(num.into());
                }

                let call_site = self
                    .builder
                    .build_call(function, &arg_values, "calltmp")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;

                match call_site.try_as_basic_value().left() {
                    Some(v) => Ok(CgValue::Number(v.into_float_value())),
                    None => Ok(CgValue::Void),
                }
            }

            Expr::Block(block) => {
                let mut last = CgValue::Void;
                for e in &block.body {
                    if self.is_current_block_terminated() {
                        break;
                    }
                    last = self.visit_expr(e)?;
                }
                Ok(last)
            }

            Expr::Let(let_expr) => {
                let function = self.current_fn()?;
                self.push_scope();

                for binding in &let_expr.bindings {
                    let value = self.eval_number(&binding.value)?;
                    let slot = self.create_entry_alloca(function, &binding.name)?;
                    self.builder
                        .build_store(slot, value)
                        .map_err(|e| CodegenError::Builder(e.to_string()))?;
                    self.symbols.insert(binding.name.clone(), slot);
                }

                let out = self.visit_expr(&let_expr.body)?;
                self.pop_scope();
                Ok(out)
            }

            Expr::If(if_expr) => {
                if !if_expr.elif_chain.is_empty() {
                    return Err(CodegenError::Unsupported(
                        "if con elif aun no implementado".to_string(),
                    ));
                }

                let function = self.current_fn()?;
                let cond = self.eval_bool(&if_expr.condition)?;

                let then_block = self.context.append_basic_block(function, "if_then");
                let else_block = self.context.append_basic_block(function, "if_else");
                let merge_block = self.ensure_merge_block(function, "if_merge");

                self.builder
                    .build_conditional_branch(cond, then_block, else_block)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;

                self.builder.position_at_end(then_block);
                let then_val = self.visit_expr(&if_expr.then_body)?;
                let then_value = self.require_number(then_val)?;
                let then_end = self.builder.get_insert_block().ok_or_else(|| {
                    CodegenError::Unsupported("if_then sin bloque actual".to_string())
                })?;
                if !self.is_current_block_terminated() {
                    self.builder
                        .build_unconditional_branch(merge_block)
                        .map_err(|e| CodegenError::Builder(e.to_string()))?;
                }

                self.builder.position_at_end(else_block);
                let else_val = self.visit_expr(&if_expr.else_body)?;
                let else_value = self.require_number(else_val)?;
                let else_end = self.builder.get_insert_block().ok_or_else(|| {
                    CodegenError::Unsupported("if_else sin bloque actual".to_string())
                })?;
                if !self.is_current_block_terminated() {
                    self.builder
                        .build_unconditional_branch(merge_block)
                        .map_err(|e| CodegenError::Builder(e.to_string()))?;
                }

                self.builder.position_at_end(merge_block);
                let phi = self
                    .builder
                    .build_phi(self.f64_type(), "iftmp")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                phi.add_incoming(&[(&then_value, then_end), (&else_value, else_end)]);
                Ok(CgValue::Number(phi.as_basic_value().into_float_value()))
            }

            Expr::While(while_expr) => {
                let function = self.current_fn()?;

                let cond_block = self.context.append_basic_block(function, "while_cond");
                let body_block = self.context.append_basic_block(function, "while_body");
                let end_block = self.context.append_basic_block(function, "while_end");

                self.builder
                    .build_unconditional_branch(cond_block)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;

                self.builder.position_at_end(cond_block);
                let cond = self.eval_bool(&while_expr.condition)?;
                self.builder
                    .build_conditional_branch(cond, body_block, end_block)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;

                self.builder.position_at_end(body_block);
                let _ = self.visit_expr(&while_expr.body)?;
                if !self.is_current_block_terminated() {
                    self.builder
                        .build_unconditional_branch(cond_block)
                        .map_err(|e| CodegenError::Builder(e.to_string()))?;
                }

                self.builder.position_at_end(end_block);
                Ok(CgValue::Void)
            }

            Expr::For(_) => Err(CodegenError::Unsupported(
                "for aun no implementado en codegen directo".to_string(),
            )),
            Expr::New(_) => Err(CodegenError::Unsupported(
                "new aun no implementado".to_string(),
            )),
            Expr::Access(_) => Err(CodegenError::Unsupported(
                "access aun no implementado".to_string(),
            )),
            Expr::MethodCall(_) => Err(CodegenError::Unsupported(
                "method_call aun no implementado".to_string(),
            )),
            Expr::Index(_) => Err(CodegenError::Unsupported(
                "index aun no implementado".to_string(),
            )),
            Expr::Is { .. } => Err(CodegenError::Unsupported(
                "operador is aun no implementado".to_string(),
            )),
            Expr::As { .. } => Err(CodegenError::Unsupported(
                "operador as aun no implementado".to_string(),
            )),
            Expr::Base(_) => Err(CodegenError::Unsupported(
                "base aun no implementado".to_string(),
            )),
            Expr::Vector(_) => Err(CodegenError::Unsupported(
                "vector aun no implementado".to_string(),
            )),
        }
    }
}
