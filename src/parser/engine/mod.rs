// src/parser/engine/mod.rs

pub mod error;
pub mod semantic_actions;
pub mod parser;

pub use error::ParseError;
pub use parser::Parser;

use crate::parser::grammar::hulk_grammar;
use crate::parser::lalr::TableBuilder;
use crate::parser::grammar::token_translator::TokenStream;
use crate::parser::ast::Program;
use crate::lexer::token::Token;

// ─────────────────────────────────────────────────────────────────────────────
//  ParserDriver
// ─────────────────────────────────────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ParserDriver {
    grammar: crate::parser::grammar::Grammar,
    table:   crate::parser::lalr::ParseTable,
}

impl ParserDriver {
    /// Construye la gramática y la tabla LALR(1).
    pub fn new() -> Self {
        let grammar = hulk_grammar::build();
        let table   = TableBuilder::new(&grammar).build();
        Self { grammar, table }
    }

    pub fn load_or_build(cache_path: &std::path::Path) -> Self {
        if let Ok(bytes) = std::fs::read(cache_path) {
            if let Ok(driver) = bincode::deserialize(&bytes) {
                return driver;
            }
        }
        let driver = Self::new();
        if let Ok(bytes) = bincode::serialize(&driver) {
            let _ = std::fs::write(cache_path, bytes);
        }
        driver
    }

    /// Parsea un iterador de tokens y devuelve el AST o un error.
    pub fn parse<I>(&self, tokens: I) -> Result<Program, ParseError>
    where
        I: Iterator<Item = Token>,
    {
        let stream = TokenStream::new(tokens);
        let parser = Parser::new(&self.grammar, &self.table);
        parser.parse(stream)
    }

    pub fn num_states(&self) -> usize { self.table.num_states }
    pub fn has_conflicts(&self) -> bool { self.table.has_conflicts() }

    /// Imprime conflictos explícitamente — llamar solo cuando se necesita diagnosticar.
    pub fn report_conflicts(&self) { self.table.dump_conflicts(); }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::token::{Token, TokenType};

    fn tok(tt: TokenType, lexeme: &str) -> Token {
        Token::new(tt, lexeme.to_string(), 1, 1, false)
    }

    // La tabla se construye UNA SOLA VEZ para todos los tests del módulo.
    // En Rust los tests corren en paralelo pero cada uno crea su propio
    // ParserDriver — si el build es lento, usar lazy_static o once_cell
    // para compartirlo. Por ahora lo dejamos simple.

    #[test]
    fn driver_builds_without_panic() {
        let driver = ParserDriver::new();
        println!("Estados LALR(1): {}", driver.num_states());
        // No asertamos sobre conflictos — la gramática HULK tiene uno conocido
        // (vector generador con '|'). Lo que importa es que no explote.
    }

    #[test]
    fn parse_literal_program() {
        let driver = ParserDriver::new();
        let tokens = vec![
            tok(TokenType::NUMBER,    "42"),
            tok(TokenType::SEMICOLON, ";"),
            tok(TokenType::EOF,       ""),
        ];
        let result = driver.parse(tokens.into_iter());
        assert!(result.is_ok(), "42; debería parsear sin error: {:?}", result);
        let program = result.unwrap();
        assert!(program.declarations.is_empty());
    }

    #[test]
    fn parse_call_program() {
        let driver = ParserDriver::new();
        let tokens = vec![
            tok(TokenType::IDENTIFIER, "print"),
            tok(TokenType::LPAREN,     "("),
            tok(TokenType::NUMBER,     "42"),
            tok(TokenType::RPAREN,     ")"),
            tok(TokenType::SEMICOLON,  ";"),
            tok(TokenType::EOF,        ""),
        ];
        let result = driver.parse(tokens.into_iter());
        assert!(result.is_ok(), "print(42); debería parsear: {:?}", result);
    }

    #[test]
    fn parse_error_on_empty_input() {
        let driver = ParserDriver::new();
        let result = driver.parse(vec![tok(TokenType::EOF, "")].into_iter());
        assert!(result.is_err(), "input vacío debe ser error");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Instancia global compartida — evita reconstruir la tabla en cada test
//
//  Uso:
//      let driver = global_driver();
//      let ast    = driver.parse(tokens)?;
// ─────────────────────────────────────────────────────────────────────────────

use std::sync::OnceLock;

static DRIVER: OnceLock<ParserDriver> = OnceLock::new();

/// Devuelve la instancia global del driver, construyéndola la primera vez.
/// Las llamadas posteriores son O(1) — solo devuelven la referencia.
pub fn global_driver() -> &'static ParserDriver {
    DRIVER.get_or_init(ParserDriver::new)
}