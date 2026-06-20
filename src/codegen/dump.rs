use inkwell::context::Context;

use crate::parser::ast::Program;

use super::context::CodegenContext;
use super::error::CodegenResult;
use super::visitor::ProgramVisitor;

pub fn emit_ir_string(program: &Program) -> CodegenResult<String> {
    let context = Context::create();
    let mut cg = CodegenContext::new(&context, "hulk_ir_dump");
    cg.visit_program(program)?;
    Ok(cg.module.print_to_string().to_string())
}
