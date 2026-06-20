use std::collections::{HashMap};

pub type StateId = usize;

#[derive(Debug, Clone, PartialEq)]
pub enum Transition {
    Epsilon,
    Char(char),
    Range(char, char),
    Dot,
}

#[derive(Debug, Clone)]
pub struct NFA {
    pub start: StateId,
    pub accept: StateId,
    pub transitions: HashMap<StateId, Vec<(Transition, StateId)>>,
}

impl NFA {
    pub fn new() -> Self {
        Self {
            start: 0,
            accept: 0,
            transitions: HashMap::new(),
        }
    }

    pub fn add_transition(&mut self, from: StateId, transition: Transition, to: StateId) {
        self.transitions
            .entry(from)
            .or_default()
            .push((transition, to));
    }
}
