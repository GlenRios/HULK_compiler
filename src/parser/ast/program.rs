use super::decl::Decl;
use super::expr::Expr;
use super::span::Span;

/// Nodo raíz del AST.
///
/// Un programa HULK es una lista de declaraciones (funciones, tipos,
/// protocolos) seguida de una única expresión global que actúa
/// como punto de entrada.
///
/// ```text
/// function tan(x) => sin(x) / cos(x);   ← Decl
/// type Point(x, y) { ... }              ← Decl
///
/// print(tan(PI));                        ← entry
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    /// Declaraciones de nivel superior, en orden de aparición.
    pub declarations: Vec<Decl>,
    /// Expresión global de entrada — siempre presente.
    pub entry: Box<Expr>,
    pub span:  Span,
}

impl Program {
    pub fn new(declarations: Vec<Decl>, entry: Expr, span: Span) -> Self {
        Self { declarations, entry: Box::new(entry), span }
    }
}