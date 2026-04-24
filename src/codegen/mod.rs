pub mod context;
pub mod dump;
pub mod error;
pub mod jit;
pub mod lower_decl;
pub mod lower_expr;
pub mod lower_program;
pub mod runtime;
pub mod symbols;
pub mod value;
pub mod visitor;

#[cfg(test)]
mod tests;

pub use dump::emit_ir_string;
pub use error::{CodegenError, CodegenResult};
pub use jit::execute_program_jit;
