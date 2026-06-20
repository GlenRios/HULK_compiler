pub mod symbol;
pub mod production;
pub mod token_translator;
pub mod hulk_grammar;

use std::collections::HashMap;
use symbol::{NonTerminal, Symbol, Terminal};
use production::Production;

// ─────────────────────────────────────────────
//  Grammar
//  Contenedor central de toda la gramática.
//  Se construye una sola vez al arrancar el
//  compilador y se pasa (por referencia) al
//  builder de la tabla LALR y al parser.
// ─────────────────────────────────────────────
#[derive(Debug)]
pub struct Grammar {
    /// Todas las producciones, indexadas por su id.
    pub productions: Vec<Production>,

    /// Símbolo inicial de la gramática aumentada (siempre NonTerminal::Start).
    pub start: NonTerminal,

    /// Índice inverso: para cada no-terminal, qué producciones lo expanden.
    /// Clave: NonTerminal  →  Vec de ids de producción.
    index: HashMap<NonTerminal, Vec<usize>>,
}

impl Grammar {
    /// Crea una gramática vacía con el símbolo inicial dado.
    pub fn new(start: NonTerminal) -> Self {
        Self {
            productions: Vec::new(),
            start,
            index: HashMap::new(),
        }
    }

    /// Registra una producción, le asigna su id y actualiza el índice.
    pub fn add(&mut self, mut prod: Production) -> usize {
        let id = self.productions.len();
        prod.id = id;
        self.index
            .entry(prod.head.clone())
            .or_default()
            .push(id);
        self.productions.push(prod);
        id
    }

    /// Añade varias producciones de golpe. Cómodo en hulk_grammar.rs.
    pub fn add_all(&mut self, prods: impl IntoIterator<Item = Production>) {
        for p in prods {
            self.add(p);
        }
    }

    // ── Consultas ────────────────────────────

    /// Ids de todas las producciones cuyo head es `nt`.
    pub fn productions_for(&self, nt: &NonTerminal) -> &[usize] {
        self.index
            .get(nt)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Producción por id (panic si el id es inválido — bug del compilador).
    pub fn production(&self, id: usize) -> &Production {
        &self.productions[id]
    }

    /// Todos los no-terminales que aparecen como head de alguna producción.
    pub fn nonterminals(&self) -> impl Iterator<Item = &NonTerminal> {
        self.index.keys()
    }

    /// Todos los terminales que aparecen en algún cuerpo de producción.
    pub fn terminals(&self) -> Vec<Terminal> {
        let mut seen = std::collections::HashSet::new();
        for prod in &self.productions {
            for sym in &prod.body {
                if let Symbol::T(t) = sym {
                    seen.insert(t.clone());
                }
            }
        }
        // Siempre incluir EOF aunque no aparezca explícitamente en cuerpos
        seen.insert(Terminal::Eof);
        let mut v: Vec<_> = seen.into_iter().collect();
        v.sort_by_key(|t| format!("{:?}", t)); // orden reproducible
        v
    }

    /// Número total de producciones.
    pub fn len(&self) -> usize {
        self.productions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.productions.is_empty()
    }

    /// Imprime todas las producciones — útil para depurar la gramática.
    pub fn dump(&self) {
        for p in &self.productions {
            println!("{}", p);
        }
    }
}