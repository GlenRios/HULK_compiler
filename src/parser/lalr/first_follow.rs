// src/parser/lalr/first_follow.rs
//
// Cálculo de los conjuntos FIRST y FOLLOW para una gramática.
//
// Estos conjuntos son la base matemática del algoritmo LALR(1):
//   FIRST(α)  → qué terminales pueden iniciar una cadena derivada de α
//   FOLLOW(A) → qué terminales pueden aparecer justo después de A
//               en alguna forma sentencial de la gramática

use std::collections::{HashMap, HashSet};

use crate::parser::grammar::Grammar;
use crate::parser::grammar::symbol::{NonTerminal, Symbol, Terminal};

// ─────────────────────────────────────────────────────────────────────────────
//  Sentinel interno para "puede derivar ε"
//  Lo representamos como None dentro de los sets; Terminal::Eof = "$"
// ─────────────────────────────────────────────────────────────────────────────

/// Un elemento de un conjunto FIRST: o un terminal concreto, o ε.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FirstElem {
    Token(Terminal),
    Epsilon,
}

// ─────────────────────────────────────────────────────────────────────────────
//  FirstFollow
//  Contiene los conjuntos precalculados para toda la gramática.
// ─────────────────────────────────────────────────────────────────────────────

pub struct FirstFollow {
    /// FIRST[NT] = conjunto de terminales (+ ε) que pueden iniciar
    /// una cadena derivada del no-terminal.
    pub first: HashMap<NonTerminal, HashSet<FirstElem>>,

    /// FOLLOW[NT] = conjunto de terminales que pueden aparecer
    /// inmediatamente después del no-terminal en alguna forma sentencial.
    /// Nunca contiene ε, pero puede contener Eof ($).
    pub follow: HashMap<NonTerminal, HashSet<Terminal>>,
}

impl FirstFollow {
    /// Calcula FIRST y FOLLOW para toda la gramática.
    /// Este es el constructor principal — llama a esto una sola vez.
    pub fn compute(grammar: &Grammar) -> Self {
        let mut ff = Self {
            first:  HashMap::new(),
            follow: HashMap::new(),
        };
        ff.compute_first(grammar);
        ff.compute_follow(grammar);
        ff
    }

    // ─────────────────────────────────────────────────────────────────────
    //  FIRST
    // ─────────────────────────────────────────────────────────────────────

    fn compute_first(&mut self, grammar: &Grammar) {
        // Inicializar todos los NT con conjunto vacío
        for nt in grammar.nonterminals() {
            self.first.insert(nt.clone(), HashSet::new());
        }

        // Iterar hasta punto fijo (ningún conjunto cambia)
        loop {
            let mut changed = false;

            for prod in &grammar.productions {
                let head = &prod.head;
                let mut new_tokens: HashSet<FirstElem> = HashSet::new();

                if prod.is_epsilon() {
                    // A → ε  ⟹  ε ∈ FIRST(A)
                    new_tokens.insert(FirstElem::Epsilon);
                } else {
                    // A → X1 X2 … Xn
                    // Añade FIRST(X1) - {ε}
                    // Si X1 puede derivar ε, añade FIRST(X2) - {ε}, etc.
                    // Si todos Xi pueden derivar ε, añade ε
                    let mut all_nullable = true;

                    for sym in &prod.body {
                        let sym_first = self.first_of_symbol(sym);

                        // Añadir terminales (no ε) al conjunto
                        for elem in &sym_first {
                            if *elem != FirstElem::Epsilon {
                                new_tokens.insert(elem.clone());
                            }
                        }

                        if !sym_first.contains(&FirstElem::Epsilon) {
                            // Este símbolo no puede derivar ε → parar
                            all_nullable = false;
                            break;
                        }
                    }

                    if all_nullable {
                        new_tokens.insert(FirstElem::Epsilon);
                    }
                }

                // Propagar al conjunto FIRST del head
                let set = self.first.entry(head.clone()).or_default();
                for elem in new_tokens {
                    if set.insert(elem) {
                        changed = true;
                    }
                }
            }

            if !changed {
                break;
            }
        }
    }

    /// FIRST de un único símbolo (terminal o no-terminal).
    fn first_of_symbol(&self, sym: &Symbol) -> HashSet<FirstElem> {
        match sym {
            Symbol::T(t) => {
                // FIRST de un terminal es el terminal mismo
                let mut set = HashSet::new();
                set.insert(FirstElem::Token(t.clone()));
                set
            }
            Symbol::NT(nt) => {
                // FIRST de un NT es el conjunto ya calculado (puede estar incompleto
                // durante la iteración, por eso repetimos hasta punto fijo)
                self.first
                    .get(nt)
                    .cloned()
                    .unwrap_or_default()
            }
        }
    }

    /// FIRST de una secuencia de símbolos α = X1 X2 … Xn.
    /// Útil para calcular lookaheads en los ítems LR(1).
    pub fn first_of_str(&self, symbols: &[Symbol]) -> HashSet<Terminal> {
        let mut result = HashSet::new();
        let mut all_nullable = true;

        for sym in symbols {
            let sym_first = self.first_of_symbol(sym);

            for elem in &sym_first {
                match elem {
                    FirstElem::Token(t) => { result.insert(t.clone()); }
                    FirstElem::Epsilon  => {}
                }
            }

            if !sym_first.contains(&FirstElem::Epsilon) {
                all_nullable = false;
                break;
            }
        }

        // Si toda la cadena puede derivar ε, no añadimos ε al resultado
        // (el llamador añadirá el lookahead externo si lo necesita)
        let _ = all_nullable;
        result
    }

    /// Como `first_of_str` pero si la cadena puede derivar ε, añade
    /// el terminal `lookahead` al resultado.
    /// Esto es exactamente lo que necesita el cálculo de clausura LR(1):
    ///   FIRST(β a)  donde a es el lookahead del ítem padre.
    pub fn first_of_str_with_lookahead(
        &self,
        symbols: &[Symbol],
        lookahead: &Terminal,
    ) -> HashSet<Terminal> {
        let mut result = HashSet::new();
        let mut all_nullable = true;

        for sym in symbols {
            let sym_first = self.first_of_symbol(sym);

            for elem in &sym_first {
                match elem {
                    FirstElem::Token(t) => { result.insert(t.clone()); }
                    FirstElem::Epsilon  => {}
                }
            }

            if !sym_first.contains(&FirstElem::Epsilon) {
                all_nullable = false;
                break;
            }
        }

        if all_nullable {
            result.insert(lookahead.clone());
        }

        result
    }

    // ─────────────────────────────────────────────────────────────────────
    //  FOLLOW
    // ─────────────────────────────────────────────────────────────────────

    fn compute_follow(&mut self, grammar: &Grammar) {
        // Inicializar todos los NT con conjunto vacío
        for nt in grammar.nonterminals() {
            self.follow.insert(nt.clone(), HashSet::new());
        }

        // Regla 1: $ ∈ FOLLOW(Start)
        self.follow
            .entry(grammar.start.clone())
            .or_default()
            .insert(Terminal::Eof);

        // Iterar hasta punto fijo
        loop {
            let mut changed = false;

            for prod in &grammar.productions {
                let head = &prod.head;

                for (i, sym) in prod.body.iter().enumerate() {
                    let Symbol::NT(nt) = sym else { continue };

                    // β = lo que viene después de `nt` en esta producción
                    let beta = &prod.body[i + 1..];

                    // Regla 2: terminales en FIRST(β) → FOLLOW(nt)
                    let first_beta = self.first_of_str(beta);
                    let follow_nt = self.follow.entry(nt.clone()).or_default();
                    for t in &first_beta {
                        if follow_nt.insert(t.clone()) {
                            changed = true;
                        }
                    }

                    // Regla 3: si β puede derivar ε (o β = vacío) → FOLLOW(head) ⊆ FOLLOW(nt)
                    let beta_nullable = beta.is_empty() || self.str_is_nullable(beta);
                    if beta_nullable {
                        // Necesitamos clonar para evitar borrow mutuo
                        let follow_head: HashSet<Terminal> = self
                            .follow
                            .get(head)
                            .cloned()
                            .unwrap_or_default();

                        let follow_nt = self.follow.entry(nt.clone()).or_default();
                        for t in follow_head {
                            if follow_nt.insert(t) {
                                changed = true;
                            }
                        }
                    }
                }
            }

            if !changed {
                break;
            }
        }
    }

    /// ¿Puede la cadena de símbolos derivar ε?
    fn str_is_nullable(&self, symbols: &[Symbol]) -> bool {
        for sym in symbols {
            let sym_first = self.first_of_symbol(sym);
            if !sym_first.contains(&FirstElem::Epsilon) {
                return false;
            }
        }
        true
    }

    // ─────────────────────────────────────────────────────────────────────
    //  Consultas públicas
    // ─────────────────────────────────────────────────────────────────────

    /// ¿Puede el no-terminal derivar ε?
    pub fn is_nullable(&self, nt: &NonTerminal) -> bool {
        self.first
            .get(nt)
            .map(|s| s.contains(&FirstElem::Epsilon))
            .unwrap_or(false)
    }

    /// FIRST de un NT — solo los terminales (sin ε).
    pub fn first_terminals(&self, nt: &NonTerminal) -> HashSet<Terminal> {
        self.first
            .get(nt)
            .into_iter()
            .flatten()
            .filter_map(|e| match e {
                FirstElem::Token(t) => Some(t.clone()),
                FirstElem::Epsilon  => None,
            })
            .collect()
    }

    /// FOLLOW de un NT.
    pub fn follow_of(&self, nt: &NonTerminal) -> &HashSet<Terminal> {
        self.follow
            .get(nt)
            .expect("NT no registrado en FOLLOW — ¿está en la gramática?")
    }

    /// Imprime todos los conjuntos — útil para depurar.
    pub fn dump(&self) {
        let mut nts: Vec<_> = self.first.keys().collect();
        nts.sort_by_key(|n| format!("{:?}", n));

        println!("=== FIRST ===");
        for nt in &nts {
            let set = &self.first[nt];
            let mut elems: Vec<_> = set.iter().map(|e| match e {
                FirstElem::Token(t) => format!("{}", t),
                FirstElem::Epsilon  => "ε".to_string(),
            }).collect();
            elems.sort();
            println!("  FIRST({:?}) = {{ {} }}", nt, elems.join(", "));
        }

        println!("=== FOLLOW ===");
        for nt in &nts {
            if let Some(set) = self.follow.get(nt) {
                let mut elems: Vec<_> = set.iter().map(|t| format!("{}", t)).collect();
                elems.sort();
                println!("  FOLLOW({:?}) = {{ {} }}", nt, elems.join(", "));
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
    use crate::parser::grammar::production::Production;
    use crate::parser::grammar::symbol::{NonTerminal::*, Terminal::*, Symbol};

    /// Gramática de ejemplo clásica para probar FIRST/FOLLOW:
    ///
    ///   S  → E $
    ///   E  → E + T
    ///   E  → T
    ///   T  → T * F
    ///   T  → F
    ///   F  → ( E )
    ///   F  → id
    ///
    /// Con los NT reutilizando:  S=Start, E=Expr, T=MulExpr, F=PrimaryExpr
    fn mini_grammar() -> Grammar {
        use crate::parser::grammar::symbol::NonTerminal as NT;
        use crate::parser::grammar::symbol::Terminal   as T;

        let mut g = Grammar::new(NT::Start);
        // 0: Start → Expr Eof
        g.add(Production::new(NT::Start, vec![
            Symbol::NT(NT::Expr), Symbol::T(T::Eof),
        ]));
        // 1: Expr → Expr + MulExpr
        g.add(Production::new(NT::Expr, vec![
            Symbol::NT(NT::Expr), Symbol::T(T::Plus), Symbol::NT(NT::MulExpr),
        ]));
        // 2: Expr → MulExpr
        g.add(Production::new(NT::Expr, vec![Symbol::NT(NT::MulExpr)]));
        // 3: MulExpr → MulExpr * PrimaryExpr
        g.add(Production::new(NT::MulExpr, vec![
            Symbol::NT(NT::MulExpr), Symbol::T(T::Multiply), Symbol::NT(NT::PrimaryExpr),
        ]));
        // 4: MulExpr → PrimaryExpr
        g.add(Production::new(NT::MulExpr, vec![Symbol::NT(NT::PrimaryExpr)]));
        // 5: PrimaryExpr → ( Expr )
        g.add(Production::new(NT::PrimaryExpr, vec![
            Symbol::T(T::LParen), Symbol::NT(NT::Expr), Symbol::T(T::RParen),
        ]));
        // 6: PrimaryExpr → IDENTIFIER
        g.add(Production::new(NT::PrimaryExpr, vec![Symbol::T(T::Identifier)]));
        g
    }

    #[test]
    fn first_of_primary_expr() {
        let g = mini_grammar();
        let ff = FirstFollow::compute(&g);
        let first = ff.first_terminals(&PrimaryExpr);
        assert!(first.contains(&LParen),     "FIRST(F) debe contener '('");
        assert!(first.contains(&Identifier), "FIRST(F) debe contener 'id'");
        assert!(!ff.is_nullable(&PrimaryExpr));
    }

    #[test]
    fn first_of_expr() {
        let g = mini_grammar();
        let ff = FirstFollow::compute(&g);
        let first = ff.first_terminals(&Expr);
        // E deriva E+T o T, T deriva T*F o F, F deriva (E) o id
        // ⟹ FIRST(E) = { (, id }
        assert!(first.contains(&LParen));
        assert!(first.contains(&Identifier));
        assert!(!ff.is_nullable(&Expr));
    }

    #[test]
    fn follow_of_expr() {
        let g = mini_grammar();
        let ff = FirstFollow::compute(&g);
        let follow = ff.follow_of(&Expr);
        // E aparece en: Start → E $, F → ( E ), E → E + T
        // FOLLOW(E) = { $, ), + }
        assert!(follow.contains(&Eof),    "FOLLOW(E) debe contener $");
        assert!(follow.contains(&RParen), "FOLLOW(E) debe contener )");
        assert!(follow.contains(&Plus),   "FOLLOW(E) debe contener +");
    }

    #[test]
    fn follow_of_mul_expr() {
        let g = mini_grammar();
        let ff = FirstFollow::compute(&g);
        let follow = ff.follow_of(&MulExpr);
        // T aparece en: E → E+T, E → T, T → T*F
        // FOLLOW(T) = FOLLOW(E) ∪ { * } = { $, ), +, * }
        assert!(follow.contains(&Eof));
        assert!(follow.contains(&RParen));
        assert!(follow.contains(&Plus));
        assert!(follow.contains(&Multiply));
    }

    #[test]
    fn nullable_nonterminal() {
        // Gramática con un NT nullable: A → ε
        let mut g = Grammar::new(Start);
        g.add(Production::new(Start, vec![Symbol::NT(Expr), Symbol::T(Eof)]));
        g.add(Production::new(Expr, vec![Symbol::NT(MulExpr)]));
        g.add(Production::epsilon(MulExpr)); // MulExpr → ε

        let ff = FirstFollow::compute(&g);
        assert!(ff.is_nullable(&MulExpr), "MulExpr debería ser nullable");
        assert!(ff.is_nullable(&Expr),   "Expr SÍ es nullable: Expr → MulExpr y MulExpr → ε, la nulabilidad se propaga");
    }

    #[test]
    fn first_of_str_with_lookahead() {
        let g = mini_grammar();
        let ff = FirstFollow::compute(&g);

        // FIRST( PrimaryExpr  ·  Eof )  con lookahead Eof
        // PrimaryExpr no es nullable → FIRST = FIRST(PrimaryExpr) = { (, id }
        let syms = vec![Symbol::NT(PrimaryExpr)];
        let result = ff.first_of_str_with_lookahead(&syms, &Eof);
        assert!(result.contains(&LParen));
        assert!(result.contains(&Identifier));
        assert!(!result.contains(&Eof), "PrimaryExpr no es nullable, no debe incluir lookahead");
    }

    #[test]
    fn first_of_empty_str_includes_lookahead() {
        let g = mini_grammar();
        let ff = FirstFollow::compute(&g);
        // FIRST( ε ·  $)  — cadena vacía → incluye el lookahead directamente
        let result = ff.first_of_str_with_lookahead(&[], &Eof);
        assert!(result.contains(&Eof));
    }

    #[test]
    fn dump_does_not_panic() {
        let g = mini_grammar();
        let ff = FirstFollow::compute(&g);
        ff.dump(); // solo verifica que no explota
    }
}