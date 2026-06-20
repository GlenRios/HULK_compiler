use crate::parser::ast::span::Span;
use super::Expr;

/// Vector literal en sus dos formas.
#[derive(Debug, Clone, PartialEq)]
pub enum VectorExpr {
    /// Forma explícita: `[e1, e2, e3]` o `[]`
    Explicit {
        elements: Vec<Expr>,
        span:     Span,
    },
    /// Forma generadora: `[expr | id in iterable]`
    Generator {
        body:     Box<Expr>,  // expresión a evaluar para cada elemento
        var:      String,     // variable de iteración
        iterable: Box<Expr>,
        span:     Span,
    },
}

impl VectorExpr {
    pub fn explicit(elements: Vec<Expr>, span: Span) -> Self {
        Self::Explicit { elements, span }
    }

    pub fn generator(body: Expr, var: impl Into<String>, iterable: Expr, span: Span) -> Self {
        Self::Generator {
            body:     Box::new(body),
            var:      var.into(),
            iterable: Box::new(iterable),
            span,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Self::Explicit  { span, .. } => *span,
            Self::Generator { span, .. } => *span,
        }
    }
}