use inkwell::IntPredicate;
use inkwell::values::BasicValue;

use crate::parser::ast::{ForExpr, IfExpr};
use crate::semantic::HulkType;

use super::super::context::CodegenContext;
use super::super::error::{CodegenError, CodegenResult};
use super::super::symbols::Place;
use super::super::value::{CgValue, ELEM_SIZE_BYTES};
use super::super::visitor::ExprVisitor;

impl<'ctx> CodegenContext<'ctx> {
    pub(super) fn lower_if(
        &mut self,
        if_expr: &IfExpr,
    ) -> CodegenResult<CgValue<'ctx>> {
        let function    = self.current_fn()?;
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
            let is_last    = i + 1 == if_expr.elif_chain.len();
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

        if incoming.is_empty() {
            self.builder.build_unreachable()
                .map_err(|e| CodegenError::Builder(e.to_string()))?;
            return Ok(CgValue::Void);
        }

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

    pub(super) fn lower_for(
        &mut self,
        fe: &ForExpr,
    ) -> CodegenResult<CgValue<'ctx>> {
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

        // i32 alloca en entry block para el índice del for
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
            _ => HulkType::Number,
        };

        let elem_val = if is_range {
            let curr_fn = self.require_fn("hulk_range_current")?;
            let num = self.builder
                .build_call(curr_fn, &[iter_ptr.into()], "rcurr")
                .map_err(|e| CodegenError::Builder(e.to_string()))?
                .try_as_basic_value().left().unwrap().into_float_value();
            CgValue::Number(num)
        } else {
            let idx    = self.builder
                .build_load(i32_ty, idx_ptr, "fidx2")
                .map_err(|e| CodegenError::Builder(e.to_string()))?.into_int_value();
            let get_fn = self.require_fn("hulk_vec_get")?;
            let ep     = self.builder
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
            let idx      = self.builder
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
}
