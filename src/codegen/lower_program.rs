use crate::parser::ast::Program;

use super::context::CodegenContext;
use super::error::{CodegenError, CodegenResult};
use super::visitor::{DeclVisitor, ExprVisitor, ProgramVisitor};

impl<'ctx> ProgramVisitor<'ctx> for CodegenContext<'ctx> {
    fn visit_program(&mut self, program: &Program) -> CodegenResult<()> {
        self.register_runtime();
        self.predeclare_functions(&program.declarations);

        for decl in &program.declarations {
            self.visit_decl(decl)?;
        }

        let entry_fn = self
            .module
            .add_function("__hulk_entry", self.f64_type().fn_type(&[], false), None);
        let entry_block = self.context.append_basic_block(entry_fn, "entry");

        self.current_function = Some(entry_fn);
        self.builder.position_at_end(entry_block);
        self.push_scope();

        let entry_value = self.visit_expr(&program.entry)?;
        let ret = self.require_number(entry_value)?;

        if !self.is_current_block_terminated() {
            self.builder
                .build_return(Some(&ret))
                .map_err(|e| CodegenError::Builder(e.to_string()))?;
        }

        self.pop_scope();
        self.current_function = None;

        self.module
            .verify()
            .map_err(|e| CodegenError::Verify(e.to_string()))?;

        Ok(())
    }
}
