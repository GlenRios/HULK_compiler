use std::fmt;

/// Posición de un nodo en el código fuente.
/// Todos los nodos del AST llevan un Span para
/// poder reportar errores con línea y columna exactas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Span {
    pub line:   usize,
    pub column: usize,
}

impl Span {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }

    /// Span vacío — usado cuando la posición no es relevante (tests, nodos sintéticos).
    pub fn dummy() -> Self {
        Self { line: 0, column: 0 }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}