use inkwell::FloatPredicate;
use inkwell::IntPredicate;
use inkwell::module::Linkage;
use inkwell::values::BasicValue;

use crate::parser::ast::{AssignExpr, AssignOp, BinaryExpr, BinaryOp, ExprKind};
use crate::semantic::HulkType;

use super::super::context::CodegenContext;
use super::super::error::{CodegenError, CodegenResult};
use super::super::symbols::Place;
use super::super::value::{CgValue, ELEM_SIZE_BYTES};
use super::super::visitor::ExprVisitor;

impl<'ctx> CodegenContext<'ctx> {
    pub(super) fn lower_binary(
        &mut self,
        bin: &BinaryExpr,
    ) -> CodegenResult<CgValue<'ctx>> {
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
                let result = self.builder
                    .build_call(pow_fn, &[l.into(), r.into()], "powtmp")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .try_as_basic_value().left()
                    .ok_or_else(|| CodegenError::Unsupported("pow no retorno valor".to_string()))?
                    .into_float_value();
                Ok(CgValue::Number(result))
            }
            BinaryOp::Eq | BinaryOp::NotEq => {
                let is_eq = matches!(bin.op, BinaryOp::Eq);
                match self.get_expr_type(&bin.left)? {
                    HulkType::Boolean => {
                        let l    = self.eval_bool(&bin.left)?;
                        let r    = self.eval_bool(&bin.right)?;
                        let pred = if is_eq { IntPredicate::EQ } else { IntPredicate::NE };
                        Ok(CgValue::Bool(
                            self.builder.build_int_compare(pred, l, r, "eqtmp")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
                    }
                    HulkType::StringT => {
                        let lv  = self.visit_expr(&bin.left)?;
                        let rv  = self.visit_expr(&bin.right)?;
                        let lp  = self.cgvalue_to_str(lv)?;
                        let rp  = self.cgvalue_to_str(rv)?;
                        let f   = self.require_fn("hulk_str_eq")?;
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
                        let lv    = self.visit_expr(&bin.left)?;
                        let rv    = self.visit_expr(&bin.right)?;
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
                        let li   = self.builder.build_ptr_to_int(lp, i64_t, "lptr")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?;
                        let ri   = self.builder.build_ptr_to_int(rp, i64_t, "rptr")
                            .map_err(|e| CodegenError::Builder(e.to_string()))?;
                        let pred = if is_eq { IntPredicate::EQ } else { IntPredicate::NE };
                        Ok(CgValue::Bool(
                            self.builder.build_int_compare(pred, li, ri, "ptreq")
                                .map_err(|e| CodegenError::Builder(e.to_string()))?,
                        ))
                    }
                    _ => {
                        let l    = self.eval_number(&bin.left)?;
                        let r    = self.eval_number(&bin.right)?;
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
                let l  = self.visit_expr(&bin.left)?;
                let r  = self.visit_expr(&bin.right)?;
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
                let l  = self.visit_expr(&bin.left)?;
                let r  = self.visit_expr(&bin.right)?;
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

    pub(super) fn lower_assign(
        &mut self,
        assign: &AssignExpr,
    ) -> CodegenResult<CgValue<'ctx>> {
        use crate::parser::ast::ExprKind;
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
            // a[i] := valor
            ExprKind::Index(ie) => {
                let coll_val = self.visit_expr(&ie.collection)?;
                let CgValue::Vector(vec_ptr) = coll_val else {
                    return Err(CodegenError::Unsupported(
                        "asignación indexada: la colección no es un Vector".into()));
                };
                let idx_val = self.visit_expr(&ie.index)?;
                let idx_f64 = self.require_number(idx_val)?;
                let i32_ty  = self.context.i32_type();
                let idx_i32 = self.builder
                    .build_float_to_signed_int(idx_f64, i32_ty, "aidx_i32")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                let get_fn   = self.require_fn("hulk_vec_get")?;
                let elem_ptr = self.builder
                    .build_call(get_fn,
                        &[vec_ptr.into(), idx_i32.into(),
                          i32_ty.const_int(ELEM_SIZE_BYTES, false).into()], "aep")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .try_as_basic_value().left()
                    .ok_or_else(|| CodegenError::Unsupported(
                        "hulk_vec_get (assign) sin retorno".into()))?
                    .into_pointer_value();
                let elem_ty = self.get_expr_type(&assign.target)?;
                Place { ptr: elem_ptr, hulk_ty: elem_ty }
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
}
