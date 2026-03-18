// src/parser/engine/parser.rs
//
// El algoritmo LR en sí: el bucle shift/reduce/accept.
//
// Estado de la máquina en cada paso:
//   stack:   Vec<(state_id, StackValue)>   — pila de (estado, valor semántico)
//   input:   TokenStream                   — flujo lazy de tokens del lexer
//   table:   &ParseTable                   — tabla ACTION/GOTO precalculada
//   grammar: &Grammar                      — para saber body_len y head de cada prod

use crate::parser::grammar::Grammar;
use crate::parser::grammar::symbol::{NonTerminal, Symbol, Terminal};
use crate::parser::grammar::token_translator::{ParserToken, TokenStream};
use crate::parser::lalr::parse_table::{Action, ParseTable};
use crate::parser::ast::{Program, Span};
use super::error::ParseError;
use super::semantic_actions::{reduce as sem_reduce, StackValue};

// ─────────────────────────────────────────────────────────────────────────────
//  Parser
// ─────────────────────────────────────────────────────────────────────────────

pub struct Parser<'a> {
    grammar: &'a Grammar,
    table:   &'a ParseTable,
}

impl<'a> Parser<'a> {
    pub fn new(grammar: &'a Grammar, table: &'a ParseTable) -> Self {
        Self { grammar, table }
    }

    /// Parsea el stream de tokens y devuelve el AST raíz o un error.
    pub fn parse<I>(&self, mut stream: TokenStream<I>) -> Result<Program, ParseError>
    where
        I: Iterator<Item = crate::lexer::token::Token>,
    {
        // Pila: cada entrada es (estado, valor semántico)
        let mut stack: Vec<(usize, StackValue)> = vec![(0, StackValue::Empty)];

        // Lookahead actual
        let mut lookahead: ParserToken = self.next_token(&mut stream);

        loop {
            let state = stack.last().unwrap().0;

            match self.table.get_action(state, &lookahead.terminal) {

                // ── Shift ────────────────────────────────────────────────
                Some(Action::Shift(next_state)) => {
                    let next_state = *next_state;
                    let val = self.token_to_value(&lookahead);
                    stack.push((next_state, val));
                    lookahead = self.next_token(&mut stream);
                }

                // ── Reduce ───────────────────────────────────────────────
                Some(Action::Reduce(prod_id)) => {
                    let prod_id = *prod_id;
                    let prod    = self.grammar.production(prod_id);

                    // El span de la reducción cubre todos los símbolos del cuerpo.
                    // Tomamos la posición del primer símbolo como aproximación.
                    let reduce_span = stack
                        .get(stack.len().saturating_sub(prod.body_len()))
                        .and_then(|(_, v)| span_of(v))
                        .unwrap_or(Span::dummy());

                    // Pop: sacar los body_len elementos y recoger sus valores
                    let mut args: Vec<StackValue> = {
                        let split_at = stack.len() - prod.body_len();
                        stack
                            .drain(split_at..)
                            .map(|(_, v)| v)
                            .collect()
                    };

                    // Construir el nodo AST para esta producción
                    let result = sem_reduce(prod_id, self.grammar, args, reduce_span)?;

                    // Goto: el estado en la cima del stack después del pop
                    let top_state = stack.last().unwrap().0;
                    let nt = &prod.head;
                    let goto_state = self
                        .table
                        .get_goto(top_state, nt)
                        .ok_or_else(|| ParseError::internal(
                            format!(
                                "GOTO[{}][{:?}] no encontrado tras reducción {}",
                                top_state, nt, prod_id
                            ),
                            reduce_span,
                        ))?;

                    stack.push((goto_state, result));
                }

                // ── Accept ───────────────────────────────────────────────
                Some(Action::Accept) => {
                    // El valor en la cima debe ser el Program construido
                    // por la producción Start → Program Eof.
                    // En la práctica el Program está un nivel abajo (lo empujó
                    // el último Reduce antes del Accept).
                    let program = self.extract_program(&mut stack)?;
                    return Ok(program);
                }

                // ── Error ────────────────────────────────────────────────
                None => {
                    let expected = self.expected_tokens(state);
                    let span = Span::new(lookahead.line, lookahead.column);

                    return Err(if lookahead.terminal == Terminal::Eof {
                        ParseError::unexpected_eof(expected, span)
                    } else {
                        ParseError::unexpected_token(
                            lookahead.terminal,
                            lookahead.lexeme,
                            expected,
                            span,
                        )
                    });
                }
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    //  Helpers
    // ─────────────────────────────────────────────────────────────────────

    /// Convierte un `ParserToken` en el `StackValue` que se empuja con Shift.
    fn token_to_value(&self, tok: &ParserToken) -> StackValue {
        let span = Span::new(tok.line, tok.column);
        match &tok.terminal {
            Terminal::True  => StackValue::Bool(true,  span),
            Terminal::False => StackValue::Bool(false, span),
            _               => StackValue::Lexeme(tok.lexeme.clone(), span),
        }
    }

    /// Pide el siguiente token al stream, o devuelve Eof si el stream terminó.
    fn next_token<I>(&self, stream: &mut TokenStream<I>) -> ParserToken
    where
        I: Iterator<Item = crate::lexer::token::Token>,
    {
        stream.next().unwrap_or(ParserToken {
            terminal: Terminal::Eof,
            lexeme:   String::new(),
            line:     0,
            column:   0,
        })
    }

    /// Terminales válidos en el estado `state` — usados para mensajes de error.
    fn expected_tokens(&self, state: usize) -> Vec<Terminal> {
        self.table
            .action
            .keys()
            .filter(|(s, _)| *s == state)
            .map(|(_, t)| t.clone())
            .collect()
    }

    /// Extrae el `Program` de la pila después de Accept.
    fn extract_program(&self, stack: &mut Vec<(usize, StackValue)>) -> Result<Program, ParseError> {
        // Buscar de arriba hacia abajo en la pila
        while let Some((_, val)) = stack.pop() {
            if let StackValue::Program(p) = val {
                return Ok(p);
            }
        }
        Err(ParseError::internal(
            "Accept sin Program en la pila",
            Span::dummy(),
        ))
    }
}

/// Intenta extraer el Span de un StackValue.
fn span_of(v: &StackValue) -> Option<Span> {
    match v {
        StackValue::Lexeme(_, s)  => Some(*s),
        StackValue::Bool(_, s)    => Some(*s),
        StackValue::Expr(e)       => Some(e.span()),
        _                         => None,
    }
}