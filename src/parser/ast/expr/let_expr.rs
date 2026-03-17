use crate::parser::ast::span::Span;
use crate::parser::ast::types::TypeName;
use super::Expr;

/// Un binding individual en un `let`: `id = expr` o `id : Type = expr`
#[derive(Debug, Clone, PartialEq)]
pub struct LetBinding {
    pub name:     String,
    pub type_ann: Option<TypeName>,  // anotación de tipo opcional
    pub value:    Box<Expr>,
    pub span:     Span,
}

impl LetBinding {
    pub fn new(
        name: impl Into<String>,
        type_ann: Option<TypeName>,
        value: Expr,
        span: Span,
    ) -> Self {
        Self { name: name.into(), type_ann, value: Box::new(value), span }
    }
}

/// Expresión let: `let b1, b2, … in body`
///
/// Múltiples bindings son azúcar para lets anidados
/// (semánticamente equivalentes), pero el AST los mantiene
/// planos para que el analizador semántico los procese en orden.
#[derive(Debug, Clone, PartialEq)]
pub struct LetExpr {
    pub bindings: Vec<LetBinding>,
    pub body:     Box<Expr>,
    pub span:     Span,
}

impl LetExpr {
    pub fn new(bindings: Vec<LetBinding>, body: Expr, span: Span) -> Self {
        Self { bindings, body: Box::new(body), span }
    }
}