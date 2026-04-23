// src/semantic/mod.rs

pub mod errors;
pub mod symbol_table;
pub mod type_system;
pub mod type_checker;

pub use errors::SemanticError;
pub use type_checker::TypeChecker;
pub use type_system::HulkType;

use crate::parser::ast::Program;

/// Punto de entrada público del análisis semántico.
///
/// ```rust
/// let program = parser.parse();
/// match semantic::analyze(&program) {
///     Ok(())       => println!("✅ Semántico OK"),
///     Err(errors)  => { for e in &errors { eprintln!("❌ {}", e); } }
/// }
/// ```
pub fn analyze(program: &Program) -> Result<(), Vec<SemanticError>> {
    let mut checker = TypeChecker::new();
    let errors = checker.check_program(program);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests;