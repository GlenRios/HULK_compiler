use inkwell::values::BasicMetadataValueEnum;

use crate::parser::ast::{CallExpr, Expr, ExprKind, MethodCallExpr};
use crate::semantic::HulkType;

use super::super::context::CodegenContext;
use super::super::error::{CodegenError, CodegenResult};
use super::super::value::CgValue;
use super::super::visitor::ExprVisitor;

impl<'ctx> CodegenContext<'ctx> {
    pub(super) fn lower_call(
        &mut self,
        call: &CallExpr,
    ) -> CodegenResult<CgValue<'ctx>> {
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
                "solo se soporta llamada directa por nombre".to_string())),
        };

        // ── Builtins especiales ───────────────────────────────────────────────
        match callee_name.as_str() {
            "print" => {
                let arg_expr = &call.args[0];
                let arg_val  = self.visit_expr(arg_expr)?;
                let str_ptr  = if let CgValue::Object(_) = arg_val {
                    let label = match self.get_expr_type(arg_expr)? {
                        HulkType::UserDefined(n) => format!("<{}>", n),
                        _ => "<Object>".to_string(),
                    };
                    self.builder
                        .build_global_string_ptr(&label, "obj_str")
                        .map_err(|e| CodegenError::Builder(e.to_string()))?
                        .as_pointer_value()
                } else {
                    self.cgvalue_to_str(arg_val)?
                };
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
                    .try_as_basic_value().left()
                    .ok_or_else(|| CodegenError::Unsupported("hulk_rand sin valor de retorno".into()))?
                    .into_float_value();
                return Ok(CgValue::Number(v));
            }
            "sqrt" | "sin" | "cos" | "exp" => {
                let arg = self.eval_number(&call.args[0])?;
                let f   = self.require_fn(&callee_name)?;
                let v   = self.builder
                    .build_call(f, &[arg.into()], "mathtmp")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .try_as_basic_value().left()
                    .ok_or_else(|| CodegenError::Unsupported(
                        format!("{} sin valor de retorno", callee_name)))?
                    .into_float_value();
                return Ok(CgValue::Number(v));
            }
            "log" => {
                let base    = self.eval_number(&call.args[0])?;
                let val     = self.eval_number(&call.args[1])?;
                let ln_fn   = self.require_fn("log")?;
                let ln_val  = self.builder
                    .build_call(ln_fn, &[val.into()],  "ln_val")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .try_as_basic_value().left()
                    .ok_or_else(|| CodegenError::Unsupported("log sin valor de retorno".into()))?
                    .into_float_value();
                let ln_base = self.builder
                    .build_call(ln_fn, &[base.into()], "ln_base")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .try_as_basic_value().left()
                    .ok_or_else(|| CodegenError::Unsupported("log(base) sin valor de retorno".into()))?
                    .into_float_value();
                let result  = self.builder
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
                    .try_as_basic_value().left()
                    .ok_or_else(|| CodegenError::Unsupported("hulk_range_alloc sin retorno".into()))?
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

    pub(super) fn lower_method_call(
        &mut self,
        mc:   &MethodCallExpr,
        expr: &Expr,
    ) -> CodegenResult<CgValue<'ctx>> {
        let obj_val = self.visit_expr(&mc.object)?;
        let obj_ptr = match obj_val {
            CgValue::Object(p) => p,
            _ => return Err(CodegenError::Unsupported(
                format!("método '{}': el receptor no es un objeto", mc.method))),
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

            _ => return Err(CodegenError::Unsupported(
                format!("método '{}': tipo de receptor no soportado por el codegen", mc.method))),
        }
    }
}
