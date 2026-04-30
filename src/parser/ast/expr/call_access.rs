use crate::parser::ast::span::Span;
use super::Expr;

/// Llamada a función o functor: `callee(args)`
#[derive(Debug, Clone, PartialEq)]
pub struct CallExpr {
    pub callee: Box<Expr>,
    pub args:   Vec<Expr>,
    pub span:   Span,
}

impl CallExpr {
    pub fn new(callee: Expr, args: Vec<Expr>, span: Span) -> Self {
        Self { callee: Box::new(callee), args, span }
    }
}

/// Acceso a atributo: `object.field`
#[derive(Debug, Clone, PartialEq)]
pub struct AccessExpr {
    pub object: Box<Expr>,
    pub field:  String,
    pub span:   Span,
}

impl AccessExpr {
    pub fn new(object: Expr, field: impl Into<String>, span: Span) -> Self {
        Self { object: Box::new(object), field: field.into(), span }
    }
}

/// Llamada a método: `object.method(args)`
///
/// Se mantiene separado de `CallExpr` + `AccessExpr` porque
/// el análisis semántico necesita resolver el método en el contexto
/// del tipo del objeto, no como una expresión standalone.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodCallExpr {
    pub object: Box<Expr>,
    pub method: String,
    pub args:   Vec<Expr>,
    pub span:   Span,
}

impl MethodCallExpr {
    pub fn new(object: Expr, method: impl Into<String>, args: Vec<Expr>, span: Span) -> Self {
        Self { object: Box::new(object), method: method.into(), args, span }
    }
}

/// Indexación: `collection[index]`
#[derive(Debug, Clone, PartialEq)]
pub struct IndexExpr {
    pub collection: Box<Expr>,
    pub index:      Box<Expr>,
    pub span:       Span,
}

impl IndexExpr {
    pub fn new(collection: Expr, index: Expr, span: Span) -> Self {
        Self { collection: Box::new(collection), index: Box::new(index), span }
    }
}
