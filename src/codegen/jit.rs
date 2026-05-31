use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::targets::{InitializationConfig, Target};

use crate::parser::ast::Program;
use crate::semantic;

use super::context::CodegenContext;
use super::error::{CodegenError, CodegenResult};
use super::runtime::{
    hulk_print, hulk_rand, hulk_str_from_number,
    hulk_str_concat, hulk_str_concat_space, hulk_str_size,
    hulk_str_eq,
    hulk_vec_alloc, hulk_vec_get, hulk_vec_size,
    hulk_range_alloc, hulk_range_next, hulk_range_current,
};
use super::visitor::ProgramVisitor;

pub fn execute_program_jit(program: &Program) -> CodegenResult<f64> {
    Target::initialize_native(&InitializationConfig::default())
        .map_err(|e| CodegenError::Jit(e.to_string()))?;

    let output = semantic::analyze(program)
        .map_err(|_| CodegenError::Unsupported("errores semanticos".to_string()))?;

    let context = Context::create();
    let mut cg = CodegenContext::from_semantic_output(&context, "hulk_jit_module", output);
    cg.visit_program(program)?;

    let ee = cg
        .module
        .create_jit_execution_engine(OptimizationLevel::None)
        .map_err(|e| CodegenError::Jit(e.to_string()))?;

    // Registrar las funciones hulk_* del runtime Rust con el JIT.
    // El JIT resuelve funciones de libm/libc automáticamente, pero las
    // funciones definidas en este binario necesitan mapeo explícito.
    let mappings: &[(&str, usize)] = &[
        ("hulk_print",            hulk_print            as *const () as usize),
        ("hulk_rand",             hulk_rand             as *const () as usize),
        ("hulk_str_from_number",  hulk_str_from_number  as *const () as usize),
        ("hulk_str_concat",       hulk_str_concat       as *const () as usize),
        ("hulk_str_concat_space", hulk_str_concat_space as *const () as usize),
        ("hulk_str_size",         hulk_str_size         as *const () as usize),
        ("hulk_str_eq",           hulk_str_eq           as *const () as usize),
        ("hulk_vec_alloc",        hulk_vec_alloc        as *const () as usize),
        ("hulk_vec_get",          hulk_vec_get          as *const () as usize),
        ("hulk_vec_size",         hulk_vec_size         as *const () as usize),
        ("hulk_range_alloc",      hulk_range_alloc      as *const () as usize),
        ("hulk_range_next",       hulk_range_next       as *const () as usize),
        ("hulk_range_current",    hulk_range_current    as *const () as usize),
    ];
    for (name, addr) in mappings {
        if let Some(fn_val) = cg.module.get_function(name) {
            ee.add_global_mapping(&fn_val, *addr);
        }
    }

    let fn_name = "__hulk_entry";

    let entry = unsafe {
        ee.get_function::<unsafe extern "C" fn() -> f64>(fn_name)
            .map_err(|e| CodegenError::Jit(e.to_string()))?
    };

    let result = unsafe { entry.call() };
    Ok(result)
}
