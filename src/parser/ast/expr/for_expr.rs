use crate::parser::ast::span::Span;
use super::Expr;

/// Bucle for: `for (id in iterable) body`
///
/// Se transpila a un `while` equivalente durante el análisis semántico
/// o la generación de código.
#[derive(Debug, Clone, PartialEq)]
pub struct ForExpr {
    pub var:      String,   // nombre de la variable de iteración
    pub iterable: Box<Expr>,
    pub body:     Box<Expr>,
    pub span:     Span,
}

impl ForExpr {
    pub fn new(var: impl Into<String>, iterable: Expr, body: Expr, span: Span) -> Self {
        Self {
            var: var.into(),
            iterable: Box::new(iterable),
            body: Box::new(body),
            span,
        }
    }
}