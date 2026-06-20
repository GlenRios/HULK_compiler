use crate::parser::ast::span::Span;
use super::Expr;

/// Bucle while: `while (cond) body`
#[derive(Debug, Clone, PartialEq)]
pub struct WhileExpr {
    pub condition: Box<Expr>,
    pub body:      Box<Expr>,
    pub span:      Span,
}

impl WhileExpr {
    pub fn new(condition: Expr, body: Expr, span: Span) -> Self {
        Self { condition: Box::new(condition), body: Box::new(body), span }
    }
}