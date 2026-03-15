use std::fmt;
use crate::parser::grammar::symbol::{NonTerminal, Symbol};

// ─────────────────────────────────────────────
//  Production
//  Representa una regla de la forma:
//      head → body[0] body[1] … body[n-1]
//
//  Cada producción tiene un id único que se
//  usa en las celdas REDUCE de la tabla LALR.
// ─────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Production {
    /// Índice único asignado al registrar la producción en la gramática.
    pub id: usize,

    /// Lado izquierdo: el no-terminal que esta regla expande.
    pub head: NonTerminal,

    /// Lado derecho: secuencia de símbolos (puede ser vacía → producción ε).
    pub body: Vec<Symbol>,
}

impl Production {
    /// Crea una producción sin id todavía (se asigna al insertarla en Grammar).
    pub fn new(head: NonTerminal, body: Vec<Symbol>) -> Self {
        Self { id: 0, head, body }
    }

    /// Producción vacía:  head → ε
    pub fn epsilon(head: NonTerminal) -> Self {
        Self { id: 0, head, body: vec![] }
    }

    /// Devuelve true si el lado derecho es vacío (producción ε).
    pub fn is_epsilon(&self) -> bool {
        self.body.is_empty()
    }

    /// Longitud del lado derecho; usada al hacer pop del stack en REDUCE.
    pub fn body_len(&self) -> usize {
        self.body.len()
    }

    /// Símbolo en la posición `pos` del cuerpo, si existe.
    pub fn symbol_at(&self, pos: usize) -> Option<&Symbol> {
        self.body.get(pos)
    }
}

impl fmt::Display for Production {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {:?} →", self.id, self.head)?;
        if self.body.is_empty() {
            write!(f, " ε")?;
        } else {
            for sym in &self.body {
                write!(f, " {}", sym)?;
            }
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────
//  Macros de conveniencia
//  Hacen la gramática mucho más legible.
//
//  Uso:
//      prod!(NT::Expr => NT::AddExpr)
//      prod!(NT::AddExpr => NT::AddExpr, T::Plus, NT::MulExpr)
//      epsilon!(NT::ElifChain)
// ─────────────────────────────────────────────

/// Construye una `Production` con cuerpo no vacío.
/// Los argumentos del cuerpo se envuelven automáticamente en `Symbol::T` o
/// `Symbol::NT` según el tipo que pases:
///   - `nt!(X)`  →  Symbol::NT(NonTerminal::X)
///   - `t!(X)`   →  Symbol::T(Terminal::X)
#[macro_export]
macro_rules! nt {
    ($v:ident) => { $crate::grammar::symbol::Symbol::NT($crate::grammar::symbol::NonTerminal::$v) };
}

#[macro_export]
macro_rules! t {
    ($v:ident) => { $crate::grammar::symbol::Symbol::T($crate::grammar::symbol::Terminal::$v) };
}

/// Crea una Production con head y cuerpo.
/// Ejemplo: `prod!(AddExpr => AddExpr, T Plus, MulExpr)`
/// Prefija con `T` los terminales, sin prefijo para no-terminales.
#[macro_export]
macro_rules! prod {
    // Cabecera + cuerpo con mezcla de T y NT
    ($head:ident => $( $sym:tt $val:ident ),+ ) => {{
        use $crate::grammar::symbol::{NonTerminal, Terminal, Symbol};
        use $crate::grammar::production::Production;
        let body = vec![
            $( prod!(@sym $sym $val) ),+
        ];
        Production::new(NonTerminal::$head, body)
    }};
    // Símbolo terminal
    (@sym T $val:ident) => { Symbol::T(Terminal::$val) };
    // Símbolo no-terminal
    (@sym NT $val:ident) => { Symbol::NT(NonTerminal::$val) };
}

/// Crea una Production vacía (ε).
#[macro_export]
macro_rules! epsilon {
    ($head:ident) => {{
        use $crate::grammar::symbol::NonTerminal;
        use $crate::grammar::production::Production;
        Production::epsilon(NonTerminal::$head)
    }};
}

// ─────────────────────────────────────────────
//  Tests unitarios
// ─────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::grammar::symbol::{NonTerminal, Terminal, Symbol};

    #[test]
    fn epsilon_production_is_empty() {
        let p = Production::epsilon(NonTerminal::ElifChain);
        assert!(p.is_epsilon());
        assert_eq!(p.body_len(), 0);
    }

    #[test]
    fn production_body_len() {
        let p = Production::new(
            NonTerminal::AddExpr,
            vec![
                Symbol::NT(NonTerminal::AddExpr),
                Symbol::T(Terminal::Plus),
                Symbol::NT(NonTerminal::MulExpr),
            ],
        );
        assert_eq!(p.body_len(), 3);
        assert!(!p.is_epsilon());
    }

    #[test]
    fn symbol_at_bounds() {
        let p = Production::new(
            NonTerminal::Expr,
            vec![Symbol::NT(NonTerminal::AddExpr)],
        );
        assert!(p.symbol_at(0).is_some());
        assert!(p.symbol_at(1).is_none());
    }

    #[test]
    fn display_normal() {
        let mut p = Production::new(
            NonTerminal::AddExpr,
            vec![
                Symbol::NT(NonTerminal::AddExpr),
                Symbol::T(Terminal::Plus),
                Symbol::NT(NonTerminal::MulExpr),
            ],
        );
        p.id = 7;
        let s = format!("{}", p);
        assert!(s.contains("[7]"));
        assert!(s.contains("+"));
    }

    #[test]
    fn display_epsilon() {
        let mut p = Production::epsilon(NonTerminal::ElifChain);
        p.id = 3;
        let s = format!("{}", p);
        assert!(s.contains("ε"));
    }
}