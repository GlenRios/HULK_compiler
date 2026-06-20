use crate::parser::ast::span::Span;
use super::Expr;

/// Una rama `elif (cond) body`
#[derive(Debug, Clone, PartialEq)]
pub struct ElifBranch {
    pub condition: Box<Expr>,
    pub body:      Box<Expr>,
    pub span:      Span,
}

impl ElifBranch {
    pub fn new(condition: Expr, body: Expr, span: Span) -> Self {
        Self { condition: Box::new(condition), body: Box::new(body), span }
    }
}

/// Expresión if: `if (cond) then [elif …]* else otherwise`
///
/// El `else` es OBLIGATORIO en HULK — `otherwise` nunca es None.
#[derive(Debug, Clone, PartialEq)]
pub struct IfExpr {
    pub condition:  Box<Expr>,
    pub then_body:  Box<Expr>,
    pub elif_chain: Vec<ElifBranch>,
    pub else_body:  Box<Expr>,
    pub span:       Span,
}

impl IfExpr {
    pub fn new(
        condition:  Expr,
        then_body:  Expr,
        elif_chain: Vec<ElifBranch>,
        else_body:  Expr,
        span:       Span,
    ) -> Self {
        Self {
            condition:  Box::new(condition),
            then_body:  Box::new(then_body),
            elif_chain,
            else_body:  Box::new(else_body),
            span,
        }
    }
}