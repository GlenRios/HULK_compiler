use inkwell::IntPredicate;

use crate::parser::ast::{TypeName, VectorExpr};
use crate::semantic::HulkType;

use super::super::context::CodegenContext;
use super::super::error::{CodegenError, CodegenResult};
use super::super::symbols::Place;
use super::super::value::{CgValue, ELEM_SIZE_BYTES};
use super::super::visitor::ExprVisitor;

impl<'ctx> CodegenContext<'ctx> {
    pub(super) fn lower_is(
        &mut self,
        inner:     &crate::parser::ast::Expr,
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

    pub(super) fn lower_vector(
        &mut self,
        ve: &VectorExpr,
    ) -> CodegenResult<CgValue<'ctx>> {
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
                    .try_as_basic_value().left()
                    .ok_or_else(|| CodegenError::Unsupported("hulk_vec_alloc sin retorno".into()))?
                    .into_pointer_value();

                for (i, elem_expr) in elements.iter().enumerate() {
                    let val      = self.visit_expr(elem_expr)?;
                    let elem_ptr = self.builder
                        .build_call(get,
                            &[vec_ptr.into(),
                              i32_ty.const_int(i as u64, false).into(),
                              i32_ty.const_int(ELEM_SIZE_BYTES, false).into()], "ep")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?
                        .try_as_basic_value().left()
                        .ok_or_else(|| CodegenError::Unsupported("hulk_vec_get sin retorno".into()))?
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
                        .try_as_basic_value().left()
                        .ok_or_else(|| CodegenError::Unsupported("hulk_vec_size sin retorno".into()))?
                        .into_float_value();
                    self.builder.build_float_to_signed_int(size_f64, i32_ty, "vcount")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?
                };

                let alloc_fn   = self.require_fn("hulk_vec_alloc")?;
                let result_ptr = self.builder
                    .build_call(alloc_fn,
                        &[count_val.into(), i32_ty.const_int(ELEM_SIZE_BYTES, false).into()], "gen_vec")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .try_as_basic_value().left()
                    .ok_or_else(|| CodegenError::Unsupported("hulk_vec_alloc (gen) sin retorno".into()))?
                    .into_pointer_value();

                // i32 alloca en entry block para el índice del generador
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

                self.builder.position_at_end(cond_bb);
                let idx  = self.builder
                    .build_load(i32_ty, idx_ptr, "idx_i32")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?.into_int_value();
                let cond = self.builder
                    .build_int_compare(IntPredicate::SLT, idx, count_val, "gen_lt")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                self.builder.build_conditional_branch(cond, body_bb, end_bb)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;

                self.builder.position_at_end(body_bb);
                self.push_scope();

                let elem_hulk_ty = match &iter_ty {
                    HulkType::Vector(t) => *t.clone(),
                    _ => HulkType::Number,
                };

                let elem_val = if is_range {
                    let next_fn = self.require_fn("hulk_range_next")?;
                    self.builder.build_call(next_fn, &[iter_ptr.into()], "")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?;
                    let curr_fn = self.require_fn("hulk_range_current")?;
                    let num = self.builder
                        .build_call(curr_fn, &[iter_ptr.into()], "rcurr")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?
                        .try_as_basic_value().left()
                        .ok_or_else(|| CodegenError::Unsupported("hulk_range_current sin retorno".into()))?
                        .into_float_value();
                    CgValue::Number(num)
                } else {
                    let get_fn = self.require_fn("hulk_vec_get")?;
                    let ep     = self.builder
                        .build_call(get_fn,
                            &[iter_ptr.into(), idx.into(),
                              i32_ty.const_int(ELEM_SIZE_BYTES, false).into()], "ep")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?
                        .try_as_basic_value().left()
                        .ok_or_else(|| CodegenError::Unsupported("hulk_vec_get (iter) sin retorno".into()))?
                        .into_pointer_value();
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
                    .try_as_basic_value().left()
                    .ok_or_else(|| CodegenError::Unsupported("hulk_vec_get (dest) sin retorno".into()))?
                    .into_pointer_value();

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
