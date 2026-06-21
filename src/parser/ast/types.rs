use std::fmt;
use super::span::Span;

/// Nombre de tipo tal como aparece en el código fuente.
///
/// ```text
/// Number          → Simple("Number")
/// Number[]        → Vector("Number")
/// Number[][]      → Vector2D("Number")   (extensión no oficial, ver A.12)
/// Number*         → Iterable("Number")
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum TypeName {
    /// Tipo nominal simple: `Number`, `Point`, `Object`
    Simple { name: String, span: Span },
    /// Tipo vector: `Number[]`
    Vector { name: String, span: Span },
    /// Tipo vector de vectores: `Number[][]`.
    /// ⚠ EXTENSIÓN NO OFICIAL — no está en la spec de HULK (A.12 solo
    /// define un nivel de `[]`). Se agrega para soportar anotaciones
    /// como `let matrix: Number[][] = ...` en tests/hulk/ok/arrays.
    Vector2D { name: String, span: Span },
    /// Tipo iterable (solo en parámetros): `Number*`
    Iterable { name: String, span: Span },
}

impl TypeName {
    pub fn simple(name: impl Into<String>, span: Span) -> Self {
        Self::Simple { name: name.into(), span }
    }

    pub fn vector(name: impl Into<String>, span: Span) -> Self {
        Self::Vector { name: name.into(), span }
    }

    pub fn vector2d(name: impl Into<String>, span: Span) -> Self {
        Self::Vector2D { name: name.into(), span }
    }

    pub fn iterable(name: impl Into<String>, span: Span) -> Self {
        Self::Iterable { name: name.into(), span }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Simple   { name, .. } => name,
            Self::Vector   { name, .. } => name,
            Self::Vector2D { name, .. } => name,
            Self::Iterable { name, .. } => name,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Self::Simple   { span, .. } => *span,
            Self::Vector   { span, .. } => *span,
            Self::Vector2D { span, .. } => *span,
            Self::Iterable { span, .. } => *span,
        }
    }
}

impl fmt::Display for TypeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Simple   { name, .. } => write!(f, "{}", name),
            Self::Vector   { name, .. } => write!(f, "{}[]", name),
            Self::Vector2D { name, .. } => write!(f, "{}[][]", name),
            Self::Iterable { name, .. } => write!(f, "{}*", name),
        }
    }
}