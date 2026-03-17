use crate::parser::ast::span::Span;
use super::Expr;

/// Operador binario infijo.
#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    // Aritmética
    Add, Sub, Mul, Div, Mod,
    Power,        // ^ o **

    // Comparación
    Eq, NotEq,
    Less, Greater,
    LessEq, GreaterEq,

    // Lógica
    And,          // &
    Or,           // |

    // Concatenación de strings
    Concat,       // @
    DoubleConcat, // @@
}

/// Expresión binaria: `left op right`
#[derive(Debug, Clone, PartialEq)]
pub struct BinaryExpr {
    pub op:    BinaryOp,
    pub left:  Box<Expr>,
    pub right: Box<Expr>,
    pub span:  Span,
}

impl BinaryExpr {
    pub fn new(op: BinaryOp, left: Expr, right: Expr, span: Span) -> Self {
        Self { op, left: Box::new(left), right: Box::new(right), span }
    }
}