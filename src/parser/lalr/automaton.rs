// src/parser/lalr/automaton.rs
//
// Construcción del Autómata Finito Determinista (AFD) de colecciones
// canónicas LR(1).
//
// Algoritmo:
//   1. Estado inicial = clausura({ [Start → • Program Eof, $] })
//   2. Para cada estado I y cada símbolo X con ítems que esperan X:
//        GOTO(I, X) = clausura({ item.advance() | item ∈ I, item espera X })
//   3. Repetir hasta que no aparezcan nuevos estados.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::parser::grammar::{Grammar, symbol::Symbol};
use crate::parser::grammar::symbol::{NonTerminal, Terminal};
use super::first_follow::FirstFollow;
use super::item::{Item, ItemSet};

// ─────────────────────────────────────────────────────────────────────────────
//  Automaton — la colección canónica LR(1) completa
// ─────────────────────────────────────────────────────────────────────────────

pub struct Automaton {
    /// Todos los estados (conjuntos de ítems), indexados por su id.
    pub states: Vec<ItemSet>,

    /// Tabla de transiciones: (estado_id, símbolo) → estado_id destino.
    pub transitions: HashMap<(usize, Symbol), usize>,
}

impl Automaton {
    /// Construye el AFD LR(1) completo para la gramática dada.
    pub fn build(grammar: &Grammar, ff: &FirstFollow) -> Self {
        let mut automaton = Self {
            states:      Vec::new(),
            transitions: HashMap::new(),
        };

        // Estado inicial: clausura del ítem kernel inicial
        // [ Start → • body[0] body[1]…, $ ]
        let start_item = Item::new(0, 0, Terminal::Eof);
        let initial_set = {
            let mut kernel = HashSet::new();
            kernel.insert(start_item);
            automaton.closure(kernel, grammar, ff)
        };

        automaton.states.push(ItemSet::new(0, initial_set));

        // Cola de estados por procesar
        let mut worklist: VecDeque<usize> = VecDeque::new();
        worklist.push_back(0);

        while let Some(state_id) = worklist.pop_front() {
            // Recoger los símbolos de transición sin mantener borrow
            let symbols: Vec<Symbol> = automaton.states[state_id]
                .transition_symbols(grammar)
                .into_iter()
                .collect();

            for sym in symbols {
                // Calcular GOTO(state_id, sym)
                let goto_items = automaton.goto(&automaton.states[state_id], &sym, grammar, ff);

                if goto_items.is_empty() {
                    continue;
                }

                // ¿Ya existe un estado con este mismo conjunto de ítems?
                let existing = automaton.states.iter().position(|s| s.items == goto_items);

                let target_id = if let Some(id) = existing {
                    id
                } else {
                    let new_id = automaton.states.len();
                    automaton.states.push(ItemSet::new(new_id, goto_items));
                    worklist.push_back(new_id);
                    new_id
                };

                automaton.transitions.insert((state_id, sym), target_id);
            }
        }

        automaton
    }

    // ─────────────────────────────────────────────────────────────────────
    //  Clausura
    //
    //  clausura(I):
    //    Para cada ítem [ A → α • B β, a ] en I
    //      Para cada producción B → γ
    //        Para cada b ∈ FIRST(β a)
    //          Añadir [ B → • γ, b ] a I
    //    Repetir hasta punto fijo
    // ─────────────────────────────────────────────────────────────────────
    fn closure(
        &self,
        kernel: HashSet<Item>,
        grammar: &Grammar,
        ff: &FirstFollow,
    ) -> HashSet<Item> {
        let mut result = kernel;
        let mut worklist: VecDeque<Item> = result.iter().cloned().collect();

        while let Some(item) = worklist.pop_front() {
            // Solo los ítems no completos generan nuevos ítems por clausura
            let Some(sym) = item.symbol_after_dot(grammar) else { continue };

            let Symbol::NT(nt) = sym else { continue };

            // β a: símbolos después del punto (excluyendo el NT que acabamos de ver)
            // + el lookahead del ítem
            let beta_a = item.beta_lookahead(grammar);

            // b ∈ FIRST(β a)
            let lookaheads = ff.first_of_str_with_lookahead(
                &beta_a[..beta_a.len().saturating_sub(1)], // β sin el lookahead
                &item.lookahead,
            );
            // Nota: beta_lookahead ya incluye el lookahead al final,
            // así que pasamos los símbolos β y el lookahead por separado:
            let beta_syms: Vec<Symbol> = {
                let prod = grammar.production(item.prod_id);
                prod.body[item.dot + 1..].to_vec()
            };
            let lookaheads = ff.first_of_str_with_lookahead(&beta_syms, &item.lookahead);

            // Para cada producción NT → γ, añadir [ NT → • γ, b ]
            for &prod_id in grammar.productions_for(nt) {
                for lookahead in &lookaheads {
                    let new_item = Item::new(prod_id, 0, lookahead.clone());
                    if result.insert(new_item.clone()) {
                        worklist.push_back(new_item);
                    }
                }
            }
        }

        result
    }

    // ─────────────────────────────────────────────────────────────────────
    //  GOTO
    //
    //  GOTO(I, X) = clausura({ item.advance() | item ∈ I, espera X })
    // ─────────────────────────────────────────────────────────────────────
    fn goto(
        &self,
        state: &ItemSet,
        sym: &Symbol,
        grammar: &Grammar,
        ff: &FirstFollow,
    ) -> HashSet<Item> {
        let kernel: HashSet<Item> = state
            .items_expecting(sym, grammar)
            .into_iter()
            .map(|item| item.advance())
            .collect();

        if kernel.is_empty() {
            return HashSet::new();
        }

        self.closure(kernel, grammar, ff)
    }

    // ─────────────────────────────────────────────────────────────────────
    //  Consultas
    // ─────────────────────────────────────────────────────────────────────

    /// Estado destino desde `state_id` consumiendo `sym`, si existe.
    pub fn get_transition(&self, state_id: usize, sym: &Symbol) -> Option<usize> {
        self.transitions.get(&(state_id, sym.clone())).copied()
    }

    /// Número total de estados del AFD.
    pub fn num_states(&self) -> usize {
        self.states.len()
    }

    /// Imprime un resumen del autómata — útil para depurar.
    pub fn dump(&self, grammar: &Grammar) {
        for state in &self.states {
            println!("=== Estado {} ===", state.id);
            let mut items: Vec<_> = state.items.iter().collect();
            items.sort_by_key(|i| (i.prod_id, i.dot, format!("{:?}", i.lookahead)));
            for item in items {
                let prod = grammar.production(item.prod_id);
                let mut body_str = String::new();
                for (i, sym) in prod.body.iter().enumerate() {
                    if i == item.dot { body_str.push_str("• "); }
                    body_str.push_str(&format!("{} ", sym));
                }
                if item.dot >= prod.body.len() { body_str.push_str("•"); }
                println!(
                    "  [{:?} → {}, {:?}]",
                    prod.head, body_str.trim(), item.lookahead
                );
            }
            // Transiciones desde este estado
            let mut trans: Vec<_> = self.transitions.iter()
                .filter(|((s, _), _)| *s == state.id)
                .collect();
            trans.sort_by_key(|((_, sym), _)| format!("{:?}", sym));
            for ((_, sym), dst) in trans {
                println!("  → {:?}  ──{}──>  Estado {}", state.id, sym, dst);
            }
        }
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
    use super::super::first_follow::FirstFollow;

    fn expr_grammar() -> Grammar {
        let mut g = Grammar::new(Start);
        g.add(Production::new(Start,      vec![Symbol::NT(Expr), Symbol::T(Eof)]));
        g.add(Production::new(Expr,       vec![Symbol::NT(Expr), Symbol::T(Plus), Symbol::NT(MulExpr)]));
        g.add(Production::new(Expr,       vec![Symbol::NT(MulExpr)]));
        g.add(Production::new(MulExpr,    vec![Symbol::T(Identifier)]));
        g
    }

    #[test]
    fn automaton_has_states() {
        let g = expr_grammar();
        let ff = FirstFollow::compute(&g);
        let auto = Automaton::build(&g, &ff);
        assert!(auto.num_states() > 0, "El autómata debe tener al menos un estado");
        println!("Estados generados: {}", auto.num_states());
    }

    #[test]
    fn initial_state_has_start_item() {
        let g = expr_grammar();
        let ff = FirstFollow::compute(&g);
        let auto = Automaton::build(&g, &ff);

        let state0 = &auto.states[0];
        let has_start = state0.items.iter().any(|item| item.prod_id == 0 && item.dot == 0);
        assert!(has_start, "El estado 0 debe contener [ Start → • Expr Eof, $ ]");
    }

    #[test]
    fn transitions_exist_from_initial() {
        let g = expr_grammar();
        let ff = FirstFollow::compute(&g);
        let auto = Automaton::build(&g, &ff);

        // Desde el estado 0 debe haber transición con Expr y con MulExpr (por clausura)
        let has_expr_trans = auto
            .get_transition(0, &Symbol::NT(Expr))
            .is_some();
        assert!(has_expr_trans, "Debe existir GOTO(0, Expr)");
    }

    #[test]
    fn dump_does_not_panic() {
        let g = expr_grammar();
        let ff = FirstFollow::compute(&g);
        let auto = Automaton::build(&g, &ff);
        auto.dump(&g);
    }
}