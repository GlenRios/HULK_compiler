use crate::parser::ast::span::Span;
use super::Expr;

/// Operador unario prefijo.
#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Neg,  // `-`  negación aritmética
    Not,  // `!`  negación lógica
}

/// Expresión unaria prefija: `-x`, `!flag`
#[derive(Debug, Clone, PartialEq)]
pub struct UnaryExpr {
    pub op:      UnaryOp,
    pub operand: Box<Expr>,
    pub span:    Span,
}

impl UnaryExpr {
    pub fn new(op: UnaryOp, operand: Expr, span: Span) -> Self {
        Self { op, operand: Box::new(operand), span }
    }
}

/// Operador postfijo.
#[derive(Debug, Clone, PartialEq)]
pub enum PostfixOp {
    Increment, // `++`
    Decrement, // `--`
}

/// Expresión postfija: `x++`, `x--`
#[derive(Debug, Clone, PartialEq)]
pub struct PostfixExpr {
    pub op:      PostfixOp,
    pub operand: Box<Expr>,
    pub span:    Span,
}

impl PostfixExpr {
    pub fn new(op: PostfixOp, operand: Expr, span: Span) -> Self {
        Self { op, operand: Box::new(operand), span }
    }
}