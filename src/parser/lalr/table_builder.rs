// src/parser/lalr/table_builder.rs

use std::collections::HashMap;
use crate::parser::grammar::{Grammar, symbol::{NonTerminal, Symbol, Terminal}};
use super::{
    automaton::Automaton,
    first_follow::FirstFollow,
    item::{Item, ItemSet},
    parse_table::{Action, ParseTable},
};
use std::collections::HashSet;

pub struct TableBuilder<'g> {
    grammar: &'g Grammar,
}

impl<'g> TableBuilder<'g> {
    pub fn new(grammar: &'g Grammar) -> Self {
        Self { grammar }
    }

    pub fn build(&self) -> ParseTable {
        let ff = FirstFollow::compute(self.grammar);
        let automaton = Automaton::build(self.grammar, &ff);

        let (merged_states, remap) = self.merge_states(&automaton);
        let merged_transitions = self.remap_transitions(&automaton, &remap);

        let mut table = ParseTable::new(merged_states.len());

        for state in &merged_states {
            self.fill_actions(state, &merged_transitions, &mut table);
        }

        table
    }

    // ── Fusión LR(1) → LALR(1) ──────────────────────────────────────────

    fn merge_states(
        &self,
        automaton: &Automaton,
    ) -> (Vec<ItemSet>, HashMap<usize, usize>) {
        // HashSet no implementa Hash, así que convertimos el core a Vec
        // ordenado para usarlo como clave del HashMap.
        // core_map: Vec<(prod_id, dot)> ordenado → índice en merged_states
        let mut core_map: HashMap<Vec<(usize, usize)>, usize> = HashMap::new();
        let mut merged: Vec<HashSet<Item>> = Vec::new();
        let mut remap: HashMap<usize, usize> = HashMap::new();

        for state in &automaton.states {
            // Convertir el core a Vec ordenado — clave hashable y determinista
            let mut core_key: Vec<(usize, usize)> = state.core().into_iter().collect();
            core_key.sort();

            if let Some(&merged_id) = core_map.get(&core_key) {
                // Fusionar lookaheads: unir los ítems del estado con los ya existentes
                for item in &state.items {
                    merged[merged_id].insert(item.clone());
                }
                remap.insert(state.id, merged_id);
            } else {
                let new_id = merged.len();
                core_map.insert(core_key, new_id);
                merged.push(state.items.clone());
                remap.insert(state.id, new_id);
            }
        }

        let merged_states: Vec<ItemSet> = merged
            .into_iter()
            .enumerate()
            .map(|(id, items)| ItemSet::new(id, items))
            .collect();

        (merged_states, remap)
    }

    fn remap_transitions(
        &self,
        automaton: &Automaton,
        remap: &HashMap<usize, usize>,
    ) -> HashMap<(usize, Symbol), usize> {
        automaton
            .transitions
            .iter()
            .map(|((from, sym), to)| {
                let new_from = remap[from];
                let new_to   = remap[to];
                ((new_from, sym.clone()), new_to)
            })
            .collect()
    }

    // ── Relleno de ACTION y GOTO ─────────────────────────────────────────

    fn fill_actions(
        &self,
        state: &ItemSet,
        transitions: &HashMap<(usize, Symbol), usize>,
        table: &mut ParseTable,
    ) {
        for item in &state.items {
            let prod = self.grammar.production(item.prod_id);

            match item.symbol_after_dot(self.grammar) {
                Some(sym) => {
                    if let Some(&target) = transitions.get(&(state.id, sym.clone())) {
                        match sym {
                            Symbol::T(t) => {
                                table.set_action(state.id, t.clone(), Action::Shift(target));
                            }
                            Symbol::NT(nt) => {
                                table.set_goto(state.id, nt.clone(), target);
                            }
                        }
                    }
                }
                None => {
                    let is_start_complete = prod.head
                        == crate::parser::grammar::symbol::NonTerminal::Start;

                    if is_start_complete {
                        table.set_action(state.id, Terminal::Eof, Action::Accept);
                    } else {
                        table.set_action(
                            state.id,
                            item.lookahead.clone(),
                            Action::Reduce(item.prod_id),
                        );
                    }
                }
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
    use super::super::parse_table::Action;

    fn expr_grammar() -> Grammar {
        let mut g = Grammar::new(Start);
        g.add(Production::new(Start,   vec![Symbol::NT(Expr), Symbol::T(Eof)]));
        g.add(Production::new(Expr,    vec![Symbol::NT(Expr), Symbol::T(Plus), Symbol::NT(MulExpr)]));
        g.add(Production::new(Expr,    vec![Symbol::NT(MulExpr)]));
        g.add(Production::new(MulExpr, vec![Symbol::T(Identifier)]));
        g
    }

    #[test]
    fn table_builds_without_panic() {
        let g = expr_grammar();
        let table = TableBuilder::new(&g).build();
        println!("Estados LALR(1): {}", table.num_states);
        table.dump_conflicts();
        assert!(!table.has_conflicts());
    }

    #[test]
    fn accept_action_exists() {
        let g = expr_grammar();
        let table = TableBuilder::new(&g).build();
        let has_accept = table.action.values().any(|a| *a == Action::Accept);
        assert!(has_accept, "Debe existir al menos una celda Accept");
    }

    #[test]
    fn shift_on_identifier_from_initial() {
        let g = expr_grammar();
        let table = TableBuilder::new(&g).build();
        let action = table.get_action(0, &Identifier);
        assert!(
            matches!(action, Some(Action::Shift(_))),
            "ACTION[0][IDENTIFIER] debe ser Shift, es: {:?}", action
        );
    }

    #[test]
    fn goto_on_expr_from_initial() {
        let g = expr_grammar();
        let table = TableBuilder::new(&g).build();
        let goto = table.get_goto(0, &Expr);
        assert!(goto.is_some(), "GOTO[0][Expr] debe existir");
    }
}