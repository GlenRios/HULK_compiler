// src/parser/engine/error.rs

use crate::parser::grammar::symbol::Terminal;
use crate::parser::ast::Span;

/// Error producido durante el parsing.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub kind:    ParseErrorKind,
    pub span:    Span,
    pub context: Option<String>,   // descripción humana del contexto, si la hay
}

#[derive(Debug, Clone)]
pub enum ParseErrorKind {
    /// Token inesperado: llegó `found`, se esperaba uno de `expected`.
    UnexpectedToken {
        found:    Terminal,
        lexeme:   String,
        expected: Vec<Terminal>,
    },
    /// Se llegó al final del archivo antes de terminar de parsear.
    UnexpectedEof {
        expected: Vec<Terminal>,
    },
    /// Error interno del parser (no debería ocurrir con una tabla correcta).
    InternalError(String),
}

impl ParseError {
    pub fn unexpected_token(
        found:    Terminal,
        lexeme:   impl Into<String>,
        expected: Vec<Terminal>,
        span:     Span,
    ) -> Self {
        Self {
            kind: ParseErrorKind::UnexpectedToken {
                found,
                lexeme: lexeme.into(),
                expected,
            },
            span,
            context: None,
        }
    }

    pub fn unexpected_eof(expected: Vec<Terminal>, span: Span) -> Self {
        Self {
            kind:    ParseErrorKind::UnexpectedEof { expected },
            span,
            context: None,
        }
    }

    pub fn internal(msg: impl Into<String>, span: Span) -> Self {
        Self {
            kind:    ParseErrorKind::InternalError(msg.into()),
            span,
            context: None,
        }
    }

    pub fn with_context(mut self, ctx: impl Into<String>) -> Self {
        self.context = Some(ctx.into());
        self
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}:{}] ", self.span.line, self.span.column)?;
        match &self.kind {
            ParseErrorKind::UnexpectedToken { found, lexeme, expected } => {
                write!(f, "token inesperado '{}' ({:?})", lexeme, found)?;
                if !expected.is_empty() {
                    let exp: Vec<_> = expected.iter().map(|t| format!("{}", t)).collect();
                    write!(f, ", se esperaba: {}", exp.join(" | "))?;
                }
            }
            ParseErrorKind::UnexpectedEof { expected } => {
                write!(f, "fin de archivo inesperado")?;
                if !expected.is_empty() {
                    let exp: Vec<_> = expected.iter().map(|t| format!("{}", t)).collect();
                    write!(f, ", se esperaba: {}", exp.join(" | "))?;
                }
            }
            ParseErrorKind::InternalError(msg) => {
                write!(f, "error interno del parser: {}", msg)?;
            }
        }
        if let Some(ctx) = &self.context {
            write!(f, " (en {})", ctx)?;
        }
        Ok(())
    }
}

impl std::error::Error for ParseError {}