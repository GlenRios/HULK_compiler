use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::targets::{InitializationConfig, Target};

use crate::parser::ast::Program;

use super::context::CodegenContext;
use super::error::{CodegenError, CodegenResult};
use super::visitor::ProgramVisitor;

pub fn execute_program_jit(program: &Program) -> CodegenResult<f64> {
    Target::initialize_native(&InitializationConfig::default())
        .map_err(|e| CodegenError::Jit(e.to_string()))?;

    let context = Context::create();
    let mut cg = CodegenContext::new(&context, "hulk_jit_module");
    cg.visit_program(program)?;

    let ee = cg
        .module
        .create_jit_execution_engine(OptimizationLevel::None)
        .map_err(|e| CodegenError::Jit(e.to_string()))?;

    let fn_name = "__hulk_entry";

    // Safety: el tipo coincide con la firma generada en lower_program.
    let entry = unsafe {
        ee.get_function::<unsafe extern "C" fn() -> f64>(fn_name)
            .map_err(|e| CodegenError::Jit(e.to_string()))?
    };

    // Safety: el puntero recuperado tiene la firma correcta.
    let result = unsafe { entry.call() };
    Ok(result)
}
