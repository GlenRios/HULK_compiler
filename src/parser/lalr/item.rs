// src/parser/lalr/item.rs
//
// Un ítem LR(1) es una producción con un punto (•) que indica cuánto
// hemos "visto" de esa producción, más un terminal de lookahead.
//
//   [ A → α • β,  a ]
//        ↑           ↑
//       punto      lookahead
//
// El punto divide el cuerpo en:
//   α = lo que ya está en el stack (lo "visto")
//   β = lo que todavía esperamos ver en la entrada
//
// El lookahead `a` es el terminal que debe aparecer en la entrada
// DESPUÉS de reducir por esta producción.

use std::collections::HashSet;
use crate::parser::grammar::symbol::{Symbol, Terminal};

// ─────────────────────────────────────────────────────────────────────────────
//  Item — un ítem LR(1)
// ─────────────────────────────────────────────────────────────────────────────

/// Un ítem LR(1): `[ prod_id, dot_pos, lookahead ]`
///
/// En lugar de almacenar una copia de la producción, guardamos su `id`
/// y consultamos la gramática cuando necesitamos los símbolos.
/// Esto mantiene los ítems pequeños y baratos de clonar.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Item {
    /// Id de la producción en la gramática.
    pub prod_id: usize,

    /// Posición del punto dentro del cuerpo.
    /// 0 = punto al inicio: [ A → • α β, a ]
    /// n = punto al final:  [ A → α β •, a ]  (ítem completo)
    pub dot: usize,

    /// Terminal de lookahead.
    pub lookahead: Terminal,
}

impl Item {
    pub fn new(prod_id: usize, dot: usize, lookahead: Terminal) -> Self {
        Self { prod_id, dot, lookahead }
    }

    /// ¿Está el punto al final? → ítem completo → candidato a REDUCE.
    pub fn is_complete(&self, grammar: &crate::parser::grammar::Grammar) -> bool {
        let prod = grammar.production(self.prod_id);
        self.dot >= prod.body.len()
    }

    /// Símbolo justo después del punto, si existe.
    /// Es el símbolo que el parser necesita ver para avanzar (Shift o Goto).
    pub fn symbol_after_dot<'g>(
        &self,
        grammar: &'g crate::parser::grammar::Grammar,
    ) -> Option<&'g Symbol> {
        grammar.production(self.prod_id).symbol_at(self.dot)
    }

    /// Nuevo ítem con el punto avanzado una posición.
    /// Solo tiene sentido llamarlo si `symbol_after_dot` es `Some`.
    pub fn advance(&self) -> Self {
        Self {
            prod_id:   self.prod_id,
            dot:       self.dot + 1,
            lookahead: self.lookahead.clone(),
        }
    }

    /// La cadena β a: los símbolos después del punto MÁS el lookahead.
    /// Usada en la clausura para calcular FIRST(β a).
    pub fn beta_lookahead<'g>(
        &self,
        grammar: &'g crate::parser::grammar::Grammar,
    ) -> Vec<Symbol> {
        let prod = grammar.production(self.prod_id);
        let mut result: Vec<Symbol> = prod.body[self.dot + 1..].to_vec();
        result.push(Symbol::T(self.lookahead.clone()));
        result
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  ItemSet — conjunto de ítems LR(1) = un estado del AFD
// ─────────────────────────────────────────────────────────────────────────────

/// Un estado del autómata LR(1) = un conjunto de ítems.
///
/// En LALR(1) dos estados que tienen el mismo "core" (mismas producciones
/// con mismas posiciones de punto, sin importar los lookaheads) se fusionan.
#[derive(Debug, Clone)]
pub struct ItemSet {
    /// Identificador único de este estado en el AFD.
    pub id: usize,
    /// Los ítems que conforman este estado.
    pub items: HashSet<Item>,
}

impl ItemSet {
    pub fn new(id: usize, items: HashSet<Item>) -> Self {
        Self { id, items }
    }

    /// El "core" de un conjunto de ítems: los ítems sin sus lookaheads.
    /// Dos estados con el mismo core se fusionan en LALR(1).
    pub fn core(&self) -> HashSet<(usize, usize)> {
        self.items
            .iter()
            .map(|item| (item.prod_id, item.dot))
            .collect()
    }

    /// Todos los símbolos que aparecen justo después del punto en algún ítem.
    /// Son las "transiciones" posibles desde este estado.
    pub fn transition_symbols(
        &self,
        grammar: &crate::parser::grammar::Grammar,
    ) -> HashSet<Symbol> {
        self.items
            .iter()
            .filter_map(|item| item.symbol_after_dot(grammar).cloned())
            .collect()
    }

    /// Ítems cuyo símbolo después del punto es exactamente `sym`.
    /// Usados para calcular GOTO(estado, sym).
    pub fn items_expecting(
        &self,
        sym: &Symbol,
        grammar: &crate::parser::grammar::Grammar,
    ) -> Vec<&Item> {
        self.items
            .iter()
            .filter(|item| item.symbol_after_dot(grammar) == Some(sym))
            .collect()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::grammar::{Grammar, production::Production};
    use crate::parser::grammar::symbol::{NonTerminal::*, Terminal::*, Symbol};

    fn make_grammar() -> Grammar {
        // E → E + T | T
        // T → id
        let mut g = Grammar::new(Start);
        g.add(Production::new(Start, vec![Symbol::NT(Expr), Symbol::T(Eof)]));
        g.add(Production::new(Expr, vec![
            Symbol::NT(Expr), Symbol::T(Plus), Symbol::NT(MulExpr),
        ]));
        g.add(Production::new(Expr, vec![Symbol::NT(MulExpr)]));
        g.add(Production::new(MulExpr, vec![Symbol::T(Identifier)]));
        g
    }

    #[test]
    fn item_not_complete_at_start() {
        let g = make_grammar();
        // [ Start → • Expr Eof, $ ]
        let item = Item::new(0, 0, Eof);
        assert!(!item.is_complete(&g));
        assert_eq!(item.symbol_after_dot(&g), Some(&Symbol::NT(Expr)));
    }

    #[test]
    fn item_complete_at_end() {
        let g = make_grammar();
        // prod 3: MulExpr → id  (body len = 1)
        // [ MulExpr → id •, $ ]
        let item = Item::new(3, 1, Eof);
        assert!(item.is_complete(&g));
        assert!(item.symbol_after_dot(&g).is_none());
    }

    #[test]
    fn item_advance() {
        let g = make_grammar();
        let item = Item::new(0, 0, Eof);   // Start → • Expr Eof
        let adv  = item.advance();          // Start → Expr • Eof
        assert_eq!(adv.dot, 1);
        assert_eq!(adv.symbol_after_dot(&g), Some(&Symbol::T(Eof)));
    }

    #[test]
    fn beta_lookahead_content() {
        let g = make_grammar();
        // prod 1: Expr → Expr • + MulExpr, con dot=1
        // beta_lookahead = [+, MulExpr, $lookahead]
        // pero dot=1 en una producción de 3 símbolos
        // beta = cuerpo[2..] = [MulExpr]   + lookahead
        let item = Item::new(1, 1, Eof);
        let bl = item.beta_lookahead(&g);
        assert_eq!(bl.len(), 2); // Plus, Eof → wait, dot=1 means after Expr
        // prod 1 body: [Expr, Plus, MulExpr]
        // dot=1 → después del punto: [Plus, MulExpr], beta = body[2..] = [MulExpr]
        // beta_lookahead = [MulExpr, Eof]
        assert_eq!(bl[0], Symbol::NT(MulExpr));
        assert_eq!(bl[1], Symbol::T(Eof));
    }

    #[test]
    fn item_set_core() {
        let mut items = HashSet::new();
        items.insert(Item::new(0, 1, Eof));
        items.insert(Item::new(0, 1, Plus)); // mismo core, distinto lookahead
        items.insert(Item::new(1, 0, Eof));

        let set = ItemSet::new(0, items);
        let core = set.core();

        // Core: {(0,1), (1,0)}  — sin lookaheads
        assert_eq!(core.len(), 2);
        assert!(core.contains(&(0, 1)));
        assert!(core.contains(&(1, 0)));
    }

    #[test]
    fn transition_symbols() {
        let g = make_grammar();
        let mut items = HashSet::new();
        // [ Start → • Expr Eof, $ ]   → espera Expr
        items.insert(Item::new(0, 0, Eof));
        // [ Expr → • Expr + MulExpr, $ ] → espera Expr
        items.insert(Item::new(1, 0, Eof));
        // [ Expr → • MulExpr, $ ]        → espera MulExpr
        items.insert(Item::new(2, 0, Eof));

        let set = ItemSet::new(0, items);
        let syms = set.transition_symbols(&g);

        assert!(syms.contains(&Symbol::NT(Expr)));
        assert!(syms.contains(&Symbol::NT(MulExpr)));
        assert_eq!(syms.len(), 2);
    }
}