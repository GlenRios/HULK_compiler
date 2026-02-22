use crate::lexer::nfa::{NFA, StateId, Transition};
use crate::lexer::regex_ast::RegexAST;
pub struct ThompsonBuilder {
    next_state: StateId,
}

impl ThompsonBuilder {
    pub fn new() -> Self {
        Self { next_state: 0 }
    }

    pub fn new_state(&mut self) -> StateId {
        let id = self.next_state;
        self.next_state += 1;
        id
    }
    pub fn build(&mut self, ast: &RegexAST) -> NFA {
        match ast {
            RegexAST::Literal(c) => self.build_literal(*c),
            RegexAST::Concat(a, b) => self.build_concat(a, b),
            RegexAST::Union(a, b) => self.build_union(a, b),
            RegexAST::Star(inner) => self.build_star(inner),
            RegexAST::Plus(inner) => self.build_plus(inner),
            RegexAST::Optional(inner) => self.build_optional(inner),
            RegexAST::Dot => self.build_dot(),
            RegexAST::Range(start, end) => self.build_range(*start, *end),
        }
    }
    fn build_literal(&mut self, c: char) -> NFA {
        let start = self.new_state();
        let accept = self.new_state();

        let mut nfa = NFA::new();
        nfa.start = start;
        nfa.accept = accept;

        nfa.add_transition(start, Transition::Char(c), accept);

        nfa
    }
    fn build_concat(&mut self, a: &RegexAST, b: &RegexAST) -> NFA {
        let mut left = self.build(a);
        let right = self.build(b);

        left.add_transition(left.accept, Transition::Epsilon, right.start);

        let mut transitions = left.transitions;
        transitions.extend(right.transitions);

        NFA {
            start: left.start,
            accept: right.accept,
            transitions,
        }
    }
    fn build_union(&mut self, a: &RegexAST, b: &RegexAST) -> NFA {
        let left = self.build(a);
        let right = self.build(b);

        let start = self.new_state();
        let accept = self.new_state();

        let mut nfa = NFA::new();
        nfa.start = start;
        nfa.accept = accept;

        nfa.transitions.extend(left.transitions);
        nfa.transitions.extend(right.transitions);

        nfa.add_transition(start, Transition::Epsilon, left.start);
        nfa.add_transition(start, Transition::Epsilon, right.start);

        nfa.add_transition(left.accept, Transition::Epsilon, accept);
        nfa.add_transition(right.accept, Transition::Epsilon, accept);

        nfa
    }
    fn build_star(&mut self, inner: &RegexAST) -> NFA {
        let sub = self.build(inner);

        let start = self.new_state();
        let accept = self.new_state();

        let mut nfa = NFA::new();
        nfa.start = start;
        nfa.accept = accept;

        nfa.transitions.extend(sub.transitions);

        nfa.add_transition(start, Transition::Epsilon, sub.start);
        nfa.add_transition(start, Transition::Epsilon, accept);

        nfa.add_transition(sub.accept, Transition::Epsilon, sub.start);
        nfa.add_transition(sub.accept, Transition::Epsilon, accept);

        nfa
    }
    fn build_plus(&mut self, inner: &RegexAST) -> NFA {
        let sub = self.build(inner);

        let start = self.new_state();
        let accept = self.new_state();

        let mut nfa = NFA::new();
        nfa.start = start;
        nfa.accept = accept;

        nfa.transitions.extend(sub.transitions);

        nfa.add_transition(start, Transition::Epsilon, sub.start);

        nfa.add_transition(sub.accept, Transition::Epsilon, sub.start);
        nfa.add_transition(sub.accept, Transition::Epsilon, accept);

        nfa
    }
    fn build_optional(&mut self, inner: &RegexAST) -> NFA {
        let sub = self.build(inner);

        let start = self.new_state();
        let accept = self.new_state();

        let mut nfa = NFA::new();
        nfa.start = start;
        nfa.accept = accept;

        nfa.transitions.extend(sub.transitions);

        nfa.add_transition(start, Transition::Epsilon, sub.start);
        nfa.add_transition(start, Transition::Epsilon, accept);

        nfa.add_transition(sub.accept, Transition::Epsilon, accept);

        nfa
    }
    fn build_dot(&mut self) -> NFA {
        let start = self.new_state();
        let accept = self.new_state();

        let mut nfa = NFA::new();
        nfa.start = start;
        nfa.accept = accept;

        nfa.add_transition(start, Transition::Dot, accept);

        nfa
    }
    fn build_range(&mut self, start_char: char, end_char: char) -> NFA {
        let start = self.new_state();
        let accept = self.new_state();

        let mut nfa = NFA::new();
        nfa.start = start;
        nfa.accept = accept;

        nfa.add_transition(start, Transition::Range(start_char, end_char), accept);

        nfa
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::regex_ast::RegexAST;

    fn count_transitions(nfa: &NFA, kind: Transition) -> usize {
        nfa.transitions
            .values()
            .flat_map(|v| v.iter())
            .filter(|(t, _)| *t == kind)
            .count()
    }

    #[test]
    fn literal_structure() {
        let ast = RegexAST::Literal('a');
        let mut builder = ThompsonBuilder::new();
        let nfa = builder.build(&ast);

        // Literal debe tener exactamente 2 estados
        assert_eq!(builder.next_state, 2);

        // Debe tener 1 transición
        assert_eq!(nfa.transitions.len(), 1);

        // Debe existir transición Char('a')
        let transitions = nfa.transitions.get(&nfa.start).unwrap();
        assert_eq!(transitions.len(), 1);

        match &transitions[0].0 {
            Transition::Char(c) => assert_eq!(*c, 'a'),
            _ => panic!("Expected Char transition"),
        }
    }

    #[test]
    fn concat_structure() {
        let ast = RegexAST::Concat(
            Box::new(RegexAST::Literal('a')),
            Box::new(RegexAST::Literal('b')),
        );

        let mut builder = ThompsonBuilder::new();
        let nfa = builder.build(&ast);

        // Concat de dos literales crea 4 estados
        assert_eq!(builder.next_state, 4);

        // Debe haber al menos una epsilon
        let epsilons = count_transitions(&nfa, Transition::Epsilon);
        assert!(epsilons >= 1);
    }

    #[test]
    fn union_structure() {
        let ast = RegexAST::Union(
            Box::new(RegexAST::Literal('a')),
            Box::new(RegexAST::Literal('b')),
        );

        let mut builder = ThompsonBuilder::new();
        let nfa = builder.build(&ast);

        // Union de dos literales crea 6 estados
        assert_eq!(builder.next_state, 6);

        // Thompson union crea 4 epsilons
        let epsilons = count_transitions(&nfa, Transition::Epsilon);
        assert_eq!(epsilons, 4);
    }

    #[test]
    fn star_structure() {
        let ast = RegexAST::Star(Box::new(RegexAST::Literal('a')));

        let mut builder = ThompsonBuilder::new();
        let nfa = builder.build(&ast);

        // Star crea 4 estados (2 del literal + 2 nuevos)
        assert_eq!(builder.next_state, 4);

        // Thompson star crea 4 epsilons
        let epsilons = count_transitions(&nfa, Transition::Epsilon);
        assert_eq!(epsilons, 4);
    }

    #[test]
    fn plus_structure() {
        let ast = RegexAST::Plus(Box::new(RegexAST::Literal('a')));

        let mut builder = ThompsonBuilder::new();
        let nfa = builder.build(&ast);

        // Plus crea 4 estados
        assert_eq!(builder.next_state, 4);

        // Debe tener al menos 2 epsilons
        let epsilons = count_transitions(&nfa, Transition::Epsilon);
        assert!(epsilons >= 2);
    }

    #[test]
    fn optional_structure() {
        let ast = RegexAST::Optional(Box::new(RegexAST::Literal('a')));

        let mut builder = ThompsonBuilder::new();
        let nfa = builder.build(&ast);

        // Optional crea 4 estados
        assert_eq!(builder.next_state, 4);

        // Debe tener 3 epsilons
        let epsilons = count_transitions(&nfa, Transition::Epsilon);
        assert_eq!(epsilons, 3);
    }
}
