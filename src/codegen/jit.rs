use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};

use crate::parser::ast::Program;
use crate::semantic::{self, SemanticOutput};

use super::context::CodegenContext;
use super::error::{CodegenError, CodegenResult};
use super::runtime::{
    hulk_print, hulk_rand, hulk_str_from_number,
    hulk_str_concat, hulk_str_concat_space, hulk_str_size,
    hulk_str_eq, hulk_type_error,
    hulk_vec_alloc, hulk_vec_get, hulk_vec_size,
    hulk_range_alloc, hulk_range_next, hulk_range_current,
};
use super::visitor::ProgramVisitor;

// ── Optimización compartida ───────────────────────────────────────────────────

const OPT_PASSES: &str =
    "mem2reg,instcombine<no-verify-fixpoint>,reassociate,simplifycfg";

fn run_opt(module: &inkwell::module::Module<'_>, machine: &TargetMachine) -> CodegenResult<()> {
    module
        .run_passes(OPT_PASSES, machine, PassBuilderOptions::create())
        .map_err(|e| CodegenError::Jit(e.to_string()))
}

fn build_target_machine(triple: &inkwell::targets::TargetTriple) -> CodegenResult<TargetMachine> {
    let target = Target::from_triple(triple)
        .map_err(|e| CodegenError::Jit(e.to_string()))?;
    target
        .create_target_machine(
            triple, "generic", "",
            OptimizationLevel::Default,
            RelocMode::Default,
            CodeModel::Default,
        )
        .ok_or_else(|| CodegenError::Jit("no se pudo crear TargetMachine".into()))
}

// ── JIT (desarrollo / tests) ──────────────────────────────────────────────────

pub fn execute_program_jit(program: &Program) -> CodegenResult<f64> {
    Target::initialize_native(&InitializationConfig::default())
        .map_err(|e| CodegenError::Jit(e.to_string()))?;

    let output = semantic::analyze(program)
        .map_err(|_| CodegenError::Unsupported("errores semánticos".to_string()))?;

    let context = Context::create();
    let mut cg = CodegenContext::from_semantic_output(&context, "hulk_jit_module", output);
    cg.visit_program(program)?;

    let triple  = TargetMachine::get_default_triple();
    let machine = build_target_machine(&triple)?;
    run_opt(&cg.module, &machine)?;

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
        ("hulk_type_error",       hulk_type_error       as *const () as usize),
    ];
    for (name, addr) in mappings {
        if let Some(fn_val) = cg.module.get_function(name) {
            ee.add_global_mapping(&fn_val, *addr);
        }
    }

    let entry = unsafe {
        ee.get_function::<unsafe extern "C" fn() -> f64>("__hulk_entry")
            .map_err(|e| CodegenError::Jit(e.to_string()))?
    };

    Ok(unsafe { entry.call() })
}

// ── AOT: compilar a objeto y linkear con gcc ──────────────────────────────────

/// Compila el programa HULK a un ejecutable ELF `./output`.
///
/// Pasos:
///  1. Codegen → módulo LLVM
///  2. Añadir wrapper `main()` que llama a `__hulk_entry`
///  3. Optimización
///  4. Emitir archivo objeto (.o) a /tmp
///  5. Linkear con hulk_runtime.a usando gcc
pub fn compile_to_binary(
    program:  &Program,
    output:   SemanticOutput,
    obj_path: &std::path::Path,
    bin_path: &std::path::Path,
    runtime:  &std::path::Path,
) -> CodegenResult<()> {
    Target::initialize_native(&InitializationConfig::default())
        .map_err(|e| CodegenError::Jit(e.to_string()))?;

    let context = Context::create();
    let mut cg = CodegenContext::from_semantic_output(&context, "hulk_module", output);
    cg.visit_program(program)?;

    // Añadir main() → llamada directa a __hulk_entry(), return 0.
    // El compilador ya emitió __hulk_entry (f64 sin args) que contiene todo
    // el programa. main() es el punto de entrada que necesita el linker de C.
    add_c_main(&context, &mut cg)?;

    let triple  = TargetMachine::get_default_triple();
    let machine = build_target_machine(&triple)?;
    run_opt(&cg.module, &machine)?;

    // Verificar IR completo (incluye main)
    cg.module
        .verify()
        .map_err(|e| CodegenError::Verify(e.to_string()))?;

    // Emitir archivo objeto
    machine
        .write_to_file(&cg.module, FileType::Object, obj_path)
        .map_err(|e| CodegenError::Jit(format!("error al escribir .o: {}", e)))?;

    // Linkear: gcc program.o hulk_runtime.a -o ./output -lm
    let status = std::process::Command::new("gcc")
        .arg(obj_path)
        .arg(runtime)
        .arg("-o")
        .arg(bin_path)
        .arg("-lm")
        .status()
        .map_err(|e| CodegenError::Jit(format!("no se pudo ejecutar gcc: {}", e)))?;

    if !status.success() {
        return Err(CodegenError::Jit(
            "gcc terminó con error al linkear el ejecutable".into(),
        ));
    }

    Ok(())
}

/// Agrega al módulo LLVM:
///   define i32 @main() { call double @__hulk_entry(); ret i32 0 }
fn add_c_main<'a>(context: &'a Context, cg: &mut CodegenContext<'a>) -> CodegenResult<()> {
    let i32_ty  = context.i32_type();
    let main_fn = cg.module.add_function("main", i32_ty.fn_type(&[], false), None);
    let bb      = context.append_basic_block(main_fn, "entry");
    cg.builder.position_at_end(bb);

    let entry_fn = cg.module
        .get_function("__hulk_entry")
        .ok_or_else(|| CodegenError::Unsupported("__hulk_entry no encontrada".into()))?;

    cg.builder
        .build_call(entry_fn, &[], "")
        .map_err(|e| CodegenError::Builder(e.to_string()))?;

    cg.builder
        .build_return(Some(&i32_ty.const_int(0, false)))
        .map_err(|e| CodegenError::Builder(e.to_string()))?;

    Ok(())
}
