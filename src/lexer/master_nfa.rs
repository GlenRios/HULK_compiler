use std::collections::{HashMap, HashSet};

use crate::lexer::nfa::{StateId, Transition};
use crate::lexer::regex_parser::RegexParser;
use crate::lexer::thompson::ThompsonBuilder;
use crate::lexer::token::TokenType;
use crate::lexer::token_definition::TokenDefinition;

#[derive(Debug)]
pub struct MasterNFA {
    pub start: StateId,
    pub transitions: HashMap<StateId, Vec<(Transition, StateId)>>,
    pub accepting: HashMap<StateId, (TokenType, bool)>,
    pub accept_order: Vec<StateId>,
}

impl MasterNFA {
    pub fn from_token_definitions(defs: &[TokenDefinition]) -> Self {
        let mut builder = ThompsonBuilder::new();

        let global_start = builder.new_state();

        let mut transitions: HashMap<StateId, Vec<(Transition, StateId)>> = HashMap::new();

        let mut accepting = HashMap::new();
        let mut accept_order = Vec::new();

        for def in defs {
            let mut parser = RegexParser::new(def.regex);
            let ast = parser.parse();

            let nfa = builder.build(&ast);

            // Conectar start global → start del token
            transitions
                .entry(global_start)
                .or_default()
                .push((Transition::Epsilon, nfa.start));

            // Copiar transiciones
            for (state, edges) in nfa.transitions {
                transitions.insert(state, edges);
            }

            // Registrar accept
            accepting.insert(nfa.accept, (def.token_type.clone(), def.skippable));

            // Guardar orden de aparición
            accept_order.push(nfa.accept);
        }

        Self {
            start: global_start,
            transitions,
            accepting,
            accept_order,
        }
    }
    pub fn epsilon_closure(&self, states: &HashSet<StateId>) -> HashSet<StateId> {
        let mut closure = HashSet::new();
        let mut stack = Vec::new();

        // Inicializar
        for &state in states {
            closure.insert(state);
            stack.push(state);
        }

        // DFS iterativo
        while let Some(state) = stack.pop() {
            if let Some(edges) = self.transitions.get(&state) {
                for (transition, target) in edges {
                    if matches!(transition, Transition::Epsilon) && !closure.contains(target) {
                        closure.insert(*target);
                        stack.push(*target);
                    }
                }
            }
        }

        closure
    }
    pub fn move_on_char(&self, states: &HashSet<StateId>, ch: char) -> HashSet<StateId> {
        let mut next_states = HashSet::new();

        for state in states {
            if let Some(edges) = self.transitions.get(state) {
                for (transition, target) in edges {
                    match transition {
                        Transition::Char(c) => {
                            if *c == ch {
                                next_states.insert(*target);
                            }
                        }

                        Transition::Range(start, end) => {
                            if ch >= *start && ch <= *end {
                                next_states.insert(*target);
                            }
                        }

                        Transition::Dot => {
                            next_states.insert(*target);
                        }

                        Transition::Epsilon => {
                            // No hacemos nada aquí.
                            // epsilon se maneja solo en epsilon_closure.
                        }
                    }
                }
            }
        }

        next_states
    }
    pub fn match_longest(
        &self,
        input: &[char],
        start_pos: usize,
    ) -> Option<(TokenType, usize, bool)> {
        let mut current_states = HashSet::new();
        current_states.insert(self.start);

        current_states = self.epsilon_closure(&current_states);

        let mut last_match: Option<(TokenType, bool)> = None;
        let mut last_match_pos = start_pos;

        let mut pos = start_pos;

        while pos < input.len() && !current_states.is_empty() {
            let ch = input[pos];

            // move
            current_states = self.move_on_char(&current_states, ch);

            // ε-closure
            current_states = self.epsilon_closure(&current_states);

            // Verificar aceptaciones
            if !current_states.is_empty() {
                // Recorremos accept_order para respetar prioridad
                for accept_state in &self.accept_order {
                    if current_states.contains(accept_state) {
                        let (token_type, skippable) = self.accepting.get(accept_state).unwrap();

                        last_match = Some((token_type.clone(), *skippable));

                        last_match_pos = pos + 1;

                        break; // IMPORTANTE: primera coincidencia gana
                    }
                }
            }

            pos += 1;
        }

        last_match.map(|(tt, sk)| (tt, last_match_pos - start_pos, sk))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_epsilon_closure_basic() {
        let mut transitions = HashMap::new();

        // 0 --ε--> 1
        transitions.insert(0, vec![(Transition::Epsilon, 1)]);

        // 1 --ε--> 2
        transitions.insert(1, vec![(Transition::Epsilon, 2)]);

        let master = MasterNFA {
            start: 0,
            transitions,
            accepting: HashMap::new(),
            accept_order: vec![],
        };

        let mut start = HashSet::new();
        start.insert(0);

        let closure = master.epsilon_closure(&start);

        assert!(closure.contains(&0));
        assert!(closure.contains(&1));
        assert!(closure.contains(&2));
        assert_eq!(closure.len(), 3);
    }

    #[test]
    fn test_move_on_char() {
        let mut transitions = HashMap::new();

        // 0 --'a'--> 1
        transitions.insert(0, vec![(Transition::Char('a'), 1)]);

        let master = MasterNFA {
            start: 0,
            transitions,
            accepting: HashMap::new(),
            accept_order: vec![],
        };

        let mut states = HashSet::new();
        states.insert(0);

        let next = master.move_on_char(&states, 'a');

        assert!(next.contains(&1));
        assert_eq!(next.len(), 1);
    }
    #[test]
    fn test_match_longest_basic() {
        let defs = vec![
            TokenDefinition {
                token_type: TokenType::KW_BASE,
                regex: "base",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::IDENTIFIER,
                regex: "[a-zA-Z]+[a-zA-Z0-9_]*",
                skippable: false,
            },
        ];

        let master = MasterNFA::from_token_definitions(&defs);

        let input: Vec<char> = "base".chars().collect();

        let result = master.match_longest(&input, 0);

        assert!(result.is_some());

        let (token_type, length, _) = result.unwrap();

        assert_eq!(token_type, TokenType::KW_BASE);
        // assert_eq!(length, 1);
    }
}
