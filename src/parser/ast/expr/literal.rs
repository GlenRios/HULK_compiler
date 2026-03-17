use crate::parser::ast::span::Span;

/// Valor literal tal como aparece en el código fuente.
/// El parser no convierte los valores — guarda el lexeme original
/// y lo pasa al análisis semántico.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    /// Número de punto flotante: `42`, `3.14`
    Number { value: String, span: Span },
    /// Cadena de texto: `"hello world"`
    String { value: String, span: Span },
    /// Carácter: `'a'`
    Char { value: String, span: Span },
    /// Booleano: `true` / `false`
    Bool { value: bool, span: Span },
    /// Nulo: `null`
    Null { span: Span },
}

impl Literal {
    pub fn span(&self) -> Span {
        match self {
            Self::Number { span, .. } => *span,
            Self::String { span, .. } => *span,
            Self::Char   { span, .. } => *span,
            Self::Bool   { span, .. } => *span,
            Self::Null   { span }     => *span,
        }
    }
}