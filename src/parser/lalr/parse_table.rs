// src/parser/lalr/parse_table.rs
//
// Las tablas ACTION y GOTO que consume el engine del parser en runtime.
//
//   ACTION[estado][terminal] → Shift(s) | Reduce(p) | Accept | Error
//   GOTO[estado][NT]         → estado siguiente (tras una reducción)

use std::collections::HashMap;
use crate::parser::grammar::symbol::{NonTerminal, Terminal};

// ─────────────────────────────────────────────────────────────────────────────
//  Action — las cuatro posibles acciones del parser LR
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Action {
    /// Desplazar: consumir el token actual y pasar al estado `s`.
    Shift(usize),

    /// Reducir: aplicar la producción con id `p`.
    /// El parser hace pop de `body_len(p)` estados del stack
    /// y luego hace Goto con el NT head de la producción.
    Reduce(usize),

    /// Aceptar: el input está completamente parseado.
    Accept,
}

// ─────────────────────────────────────────────────────────────────────────────
//  ConflictKind — tipo de conflicto detectado al construir la tabla
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ConflictKind {
    /// Dos acciones distintas para el mismo (estado, terminal).
    ShiftReduce { state: usize, terminal: Terminal, shift_to: usize, reduce_prod: usize },
    ReduceReduce { state: usize, terminal: Terminal, prod1: usize, prod2: usize },
}

// ─────────────────────────────────────────────────────────────────────────────
//  ParseTable — las tablas finales
// ─────────────────────────────────────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ParseTable {
    /// ACTION[estado][terminal] → acción
    pub action: HashMap<(usize, Terminal), Action>,

    /// GOTO[estado][NT] → estado destino
    pub goto: HashMap<(usize, NonTerminal), usize>,

    /// Conflictos detectados durante la construcción.
    /// Una gramática LALR(1) limpia tendrá este vector vacío.
    pub conflicts: Vec<ConflictKind>,

    /// Número de estados del autómata.
    pub num_states: usize,
}

impl ParseTable {
    pub fn new(num_states: usize) -> Self {
        Self {
            action:     HashMap::new(),
            goto:       HashMap::new(),
            conflicts:  Vec::new(),
            num_states,
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    //  Inserción con detección de conflictos
    // ─────────────────────────────────────────────────────────────────────

    /// Inserta una entrada en ACTION.
    /// Si ya existe una entrada distinta, registra el conflicto y
    /// aplica la regla de desambiguación por defecto:
    ///   - Shift/Reduce → preferir Shift  (más común y generalmente correcto)
    ///   - Reduce/Reduce → preferir la producción de menor id (más temprana)
    pub fn set_action(&mut self, state: usize, terminal: Terminal, action: Action) {
        let key = (state, terminal.clone());

        if let Some(existing) = self.action.get(&key) {
            if *existing == action {
                return; // misma acción, no es conflicto
            }

            match (existing, &action) {
                (Action::Shift(s), Action::Reduce(p)) => {
                    self.conflicts.push(ConflictKind::ShiftReduce {
                        state, terminal, shift_to: *s, reduce_prod: *p,
                    });
                    // Preferir Shift — no sobreescribir
                    return;
                }
                (Action::Reduce(p), Action::Shift(s)) => {
                    self.conflicts.push(ConflictKind::ShiftReduce {
                        state, terminal: terminal.clone(), shift_to: *s, reduce_prod: *p,
                    });
                    // Preferir Shift — sobreescribir con la acción nueva (Shift)
                    self.action.insert(key, action);
                    return;
                }
                (Action::Reduce(p1), Action::Reduce(p2)) => {
                    let (p1, p2) = (*p1, *p2);
                    self.conflicts.push(ConflictKind::ReduceReduce {
                        state, terminal: terminal.clone(), prod1: p1, prod2: p2,
                    });
                    // Preferir la producción de menor id
                    if p2 < p1 {
                        self.action.insert(key, action);
                    }
                    return;
                }
                _ => {}
            }
        }

        self.action.insert(key, action);
    }

    pub fn set_goto(&mut self, state: usize, nt: NonTerminal, target: usize) {
        self.goto.insert((state, nt), target);
    }

    // ─────────────────────────────────────────────────────────────────────
    //  Consultas en runtime (llamadas por el engine en cada paso)
    // ─────────────────────────────────────────────────────────────────────

    pub fn get_action(&self, state: usize, terminal: &Terminal) -> Option<&Action> {
        self.action.get(&(state, terminal.clone()))
    }

    pub fn get_goto(&self, state: usize, nt: &NonTerminal) -> Option<usize> {
        self.goto.get(&(state, nt.clone())).copied()
    }

    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }

    // ─────────────────────────────────────────────────────────────────────
    //  Depuración
    // ─────────────────────────────────────────────────────────────────────

    pub fn dump_conflicts(&self) {
        if self.conflicts.is_empty() {
            println!("Sin conflictos — gramática LALR(1) limpia.");
            return;
        }
        println!("{} conflicto(s):", self.conflicts.len());
        for c in &self.conflicts {
            match c {
                ConflictKind::ShiftReduce { state, terminal, shift_to, reduce_prod } => {
                    println!(
                        "  [S/R] Estado {}, token '{}': Shift→{} vs Reduce(prod {})",
                        state, terminal, shift_to, reduce_prod
                    );
                }
                ConflictKind::ReduceReduce { state, terminal, prod1, prod2 } => {
                    println!(
                        "  [R/R] Estado {}, token '{}': Reduce({}) vs Reduce({})",
                        state, terminal, prod1, prod2
                    );
                }
            }
        }
    }

    pub fn dump_action(&self, grammar: &crate::parser::grammar::Grammar) {
        let mut entries: Vec<_> = self.action.iter().collect();
        entries.sort_by_key(|((s, t), _)| (*s, format!("{:?}", t)));
        println!("=== ACTION table ({} entradas) ===", entries.len());
        for ((state, terminal), action) in entries {
            let action_str = match action {
                Action::Shift(s)  => format!("Shift({})", s),
                Action::Reduce(p) => {
                    let prod = grammar.production(*p);
                    format!("Reduce({}: {:?} → {:?})", p, prod.head, prod.body.len())
                }
                Action::Accept    => "Accept".to_string(),
            };
            println!("  ACTION[{}][{}] = {}", state, terminal, action_str);
        }
    }
}