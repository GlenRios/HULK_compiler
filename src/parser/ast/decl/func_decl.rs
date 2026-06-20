use crate::parser::ast::span::Span;
use crate::parser::ast::types::TypeName;
use crate::parser::ast::expr::Expr;

/// Parámetro de función/método: `id` o `id : TypeName`
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name:     String,
    pub type_ann: Option<TypeName>,
    pub span:     Span,
}

impl Param {
    pub fn new(name: impl Into<String>, type_ann: Option<TypeName>, span: Span) -> Self {
        Self { name: name.into(), type_ann, span }
    }
}

/// Declaración de función global.
///
/// ```text
/// function tan(x: Number): Number => sin(x) / cos(x);
/// function operate(x, y) { print(x + y); print(x - y); }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct FuncDecl {
    pub name:        String,
    pub params:      Vec<Param>,
    pub return_type: Option<TypeName>,
    pub body:        Box<Expr>,   // siempre una Expr (inline) o Block
    pub span:        Span,
}

impl FuncDecl {
    pub fn new(
        name:        impl Into<String>,
        params:      Vec<Param>,
        return_type: Option<TypeName>,
        body:        Expr,
        span:        Span,
    ) -> Self {
        Self { name: name.into(), params, return_type, body: Box::new(body), span }
    }
}