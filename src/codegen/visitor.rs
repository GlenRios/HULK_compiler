use crate::parser::ast::{Decl, Expr, Program};

use super::error::CodegenResult;
use super::value::CgValue;

pub trait ProgramVisitor<'ctx> {
    fn visit_program(&mut self, program: &Program) -> CodegenResult<()>;
}

pub trait DeclVisitor<'ctx> {
    fn visit_decl(&mut self, decl: &Decl) -> CodegenResult<()>;
}

pub trait ExprVisitor<'ctx> {
    fn visit_expr(&mut self, expr: &Expr) -> CodegenResult<CgValue<'ctx>>;
}
