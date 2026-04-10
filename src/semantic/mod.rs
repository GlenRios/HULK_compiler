pub mod errors;
pub mod symbol_table;
pub mod type_system;
pub mod type_checker;

pub use errors::SemanticError;
pub use type_checker::TypeChecker;

use crate::parser::ast::Program;

/// Punto de entrada público del análisis semántico
pub fn analyze(program: &Program) -> Result<(), Vec<SemanticError>> {
    let mut checker = TypeChecker::new();
    let errors = checker.check_program(program);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}