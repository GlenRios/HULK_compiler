use crate::parser::ast::span::Span;
use crate::parser::ast::types::TypeName;
use super::Expr;

/// Instanciación de tipo: `new TypeName(args)`
#[derive(Debug, Clone, PartialEq)]
pub struct NewExpr {
    pub type_name: TypeName,
    pub args:      Vec<Expr>,
    pub span:      Span,
}

impl NewExpr {
    pub fn new(type_name: TypeName, args: Vec<Expr>, span: Span) -> Self {
        Self { type_name, args, span }
    }
}