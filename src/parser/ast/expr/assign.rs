use crate::parser::ast::span::Span;
use super::Expr;

/// Operador de asignación.
#[derive(Debug, Clone, PartialEq)]
pub enum AssignOp {
    Assign,      // `:=`
    PlusAssign,  // `+=`
    MinusAssign, // `-=`
    MulAssign,   // `*=`
    DivAssign,   // `/=`
    ModAssign,   // `%=`
}

/// Asignación destructiva: `target := value`
///
/// `target` puede ser cualquier expresión sintácticamente, pero
/// semánticamente debe ser un lvalue válido (variable, atributo).
/// Esa restricción la aplica el analizador semántico.
#[derive(Debug, Clone, PartialEq)]
pub struct AssignExpr {
    pub op:     AssignOp,
    pub target: Box<Expr>,
    pub value:  Box<Expr>,
    pub span:   Span,
}

impl AssignExpr {
    pub fn new(op: AssignOp, target: Expr, value: Expr, span: Span) -> Self {
        Self { op, target: Box::new(target), value: Box::new(value), span }
    }
}