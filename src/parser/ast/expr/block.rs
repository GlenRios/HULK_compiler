use crate::parser::ast::span::Span;
use super::Expr;

/// Bloque de expresiones: `{ e1; e2; e3 }`
///
/// El valor del bloque es la última expresión.
/// La lista `body` nunca está vacía — la gramática lo garantiza.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockExpr {
    pub body: Vec<Expr>,
    pub span: Span,
}

impl BlockExpr {
    pub fn new(body: Vec<Expr>, span: Span) -> Self {
        Self { body, span }
    }

    /// Última expresión del bloque — su tipo es el tipo del bloque.
    pub fn tail(&self) -> &Expr {
        self.body.last().expect("BlockExpr never empty")
    }
}