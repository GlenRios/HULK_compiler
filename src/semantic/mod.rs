// src/semantic/mod.rs

pub mod errors;
pub mod symbol_table;
pub mod type_system;
pub mod type_checker;

pub use errors::SemanticError;
pub use type_checker::TypeChecker;
pub use type_system::{FuncSignature, HulkType, TypeHierarchy};

use std::collections::HashMap;
use crate::parser::ast::Program;

pub struct SemanticOutput {
    pub hierarchy:  TypeHierarchy,
    pub functions:  HashMap<String, FuncSignature>,
    pub expr_types: HashMap<u32, HulkType>,
}

pub fn analyze(program: &Program) -> Result<SemanticOutput, Vec<SemanticError>> {
    let mut checker = TypeChecker::new();
    let errors = checker.check_program(program);
    if errors.is_empty() {
        Ok(SemanticOutput {
            hierarchy:  checker.types,
            functions:  checker.functions,
            expr_types: checker.expr_types,
        })
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests;