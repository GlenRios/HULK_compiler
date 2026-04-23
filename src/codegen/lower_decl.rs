use inkwell::values::BasicValue;

use crate::parser::ast::{Decl, FuncDecl};

use super::context::CodegenContext;
use super::error::{CodegenError, CodegenResult};
use super::value::CgValue;
use super::visitor::{DeclVisitor, ExprVisitor};

impl<'ctx> CodegenContext<'ctx> {
    pub fn predeclare_functions(&mut self, decls: &[Decl]) {
        for decl in decls {
            if let Decl::Function(func) = decl {
                let param_types = vec![self.f64_type().into(); func.params.len()];
                let fn_type = self.f64_type().fn_type(&param_types, false);
                let function = self.module.add_function(&func.name, fn_type, None);
                self.functions.insert(func.name.clone(), function);
            }
        }
    }

    fn lower_function_decl(&mut self, func_decl: &FuncDecl) -> CodegenResult<()> {
        let function = self
            .functions
            .get(&func_decl.name)
            .copied()
            .ok_or_else(|| CodegenError::UnknownFunction(func_decl.name.clone()))?;

        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);

        self.current_function = Some(function);
        self.push_scope();

        for (idx, param_decl) in func_decl.params.iter().enumerate() {
            if let Some(param) = function.get_nth_param(idx as u32) {
                let param_val = param.into_float_value();
                let alloca = self.create_entry_alloca(function, &param_decl.name)?;
                self.builder
                    .build_store(alloca, param_val)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                self.symbols.insert(param_decl.name.clone(), alloca);
            }
        }

        let body_value = self.visit_expr(&func_decl.body)?;

        if !self.is_current_block_terminated() {
            let ret = self.require_number(body_value)?;
            self.builder
                .build_return(Some(&ret))
                .map_err(|e| CodegenError::Builder(e.to_string()))?;
        }

        self.pop_scope();
        self.current_function = None;
        Ok(())
    }
}

impl<'ctx> DeclVisitor<'ctx> for CodegenContext<'ctx> {
    fn visit_decl(&mut self, decl: &Decl) -> CodegenResult<()> {
        match decl {
            Decl::Function(func) => self.lower_function_decl(func),
            Decl::Type(_) => Err(CodegenError::Unsupported(
                "codegen de TypeDecl aun no implementado".to_string(),
            )),
            Decl::Protocol(_) => Err(CodegenError::Unsupported(
                "codegen de ProtocolDecl aun no implementado".to_string(),
            )),
        }
    }
}
