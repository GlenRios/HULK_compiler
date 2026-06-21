use crate::parser::ast::span::Span;
use crate::parser::ast::types::TypeName;
use super::Expr;

/// Vector literal en sus formas.
#[derive(Debug, Clone, PartialEq)]
pub enum VectorExpr {
    /// Forma explícita: `[e1, e2, e3]` o `[]` — también `{e1, e2, e3}`
    /// (alias de llaves, ver VectorLiteral en la gramática).
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
    /// Forma de creación con tamaño: `new Type[N]` o `new Type[N]{ id -> expr }`.
    Alloc {
        elem_type: TypeName,                     // tipo de cada elemento
        size:      Box<Expr>,                    // cantidad de elementos
        generator: Option<(String, Box<Expr>)>,  // (nombre del índice, cuerpo) si hay `{ id -> expr }`
        span:      Span,
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

    pub fn alloc(
        elem_type: TypeName,
        size: Expr,
        generator: Option<(String, Expr)>,
        span: Span,
    ) -> Self {
        Self::Alloc {
            elem_type,
            size: Box::new(size),
            generator: generator.map(|(name, body)| (name, Box::new(body))),
            span,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Self::Explicit  { span, .. } => *span,
            Self::Generator { span, .. } => *span,
            Self::Alloc     { span, .. } => *span,
        }
    }
}
