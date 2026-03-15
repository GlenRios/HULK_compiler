use crate::lexer::token::TokenType;
use crate::parser::grammar::symbol::Terminal;

// ─────────────────────────────────────────────
//  TokenTranslator
//  Convierte el TokenType del lexer al Terminal
//  que entiende el parser.
//
//  Reglas de traducción:
//   • WHITESPACE / COMMENT / NEWLINE → None
//     (el parser los ignora; el lexer ya los
//      marca como `skippable = true` pero aquí
//      hacemos el filtrado explícito igualmente)
//   • EOF → Terminal::Eof
//   • ERROR → None  (se reporta en el lexer;
//     el parser recibirá el siguiente token válido
//     o se detendrá si el stream se agota)
//   • Todo lo demás → Some(Terminal::…)
// ─────────────────────────────────────────────

pub fn token_to_terminal(token_type: &TokenType) -> Option<Terminal> {
    let terminal = match token_type {
        // ── Especiales ───────────────────────
        TokenType::EOF       => Terminal::Eof,
        TokenType::WHITESPACE
        | TokenType::COMMENT
        | TokenType::NEWLINE => return None,   // ignorar
        TokenType::ERROR     => return None,   // ignorar (ya reportado por lexer)

        // ── Literales ────────────────────────
        TokenType::NUMBER    => Terminal::Number,
        TokenType::STRING    => Terminal::String,
        TokenType::CHAR      => Terminal::Char,
        TokenType::T_NULL    => Terminal::Null,
        TokenType::TRUE      => Terminal::True,
        TokenType::FALSE     => Terminal::False,

        // ── Keywords ─────────────────────────
        TokenType::KW_IF       => Terminal::If,
        TokenType::KW_ELIF     => Terminal::Elif,
        TokenType::KW_ELSE     => Terminal::Else,
        TokenType::KW_WHILE    => Terminal::While,
        TokenType::KW_FOR      => Terminal::For,
        TokenType::KW_FUNCTION => Terminal::Function,
        TokenType::KW_LET      => Terminal::Let,
        TokenType::KW_IN       => Terminal::In,
        TokenType::KW_TYPE     => Terminal::Type,
        TokenType::KW_NEW      => Terminal::New,
        TokenType::KW_INHERITS => Terminal::Inherits,
        TokenType::KW_BASE     => Terminal::Base,
        TokenType::KW_PROTOCOL => Terminal::Protocol,
        TokenType::KW_EXTENDS  => Terminal::Extends,

        // ── Operadores de palabra ─────────────
        TokenType::OP_IS => Terminal::Is,
        TokenType::OP_AS => Terminal::As,

        // ── Operadores multicarácter ──────────
        TokenType::OP_POWER_DOUBLE_STAR => Terminal::PowerStar,
        TokenType::OP_INCREMENT         => Terminal::Increment,
        TokenType::OP_DECREMENT         => Terminal::Decrement,
        TokenType::OP_DOBLE_CONCAT      => Terminal::DoubleConcat,
        TokenType::OP_DESTRUCT_ASSIGN   => Terminal::DestructAssign,
        TokenType::OP_PLUS_ASSIGN       => Terminal::PlusAssign,
        TokenType::OP_MINUS_ASSIGN      => Terminal::MinusAssign,
        TokenType::OP_MULT_ASSIGN       => Terminal::MultAssign,
        TokenType::OP_DIV_ASSIGN        => Terminal::DivAssign,
        TokenType::OP_MOD_ASSIGN        => Terminal::ModAssign,
        TokenType::OP_EQUAL             => Terminal::Equal,
        TokenType::OP_NOT_EQUAL         => Terminal::NotEqual,
        TokenType::OP_LESS_EQ           => Terminal::LessEq,
        TokenType::OP_GREATER_EQ        => Terminal::GreaterEq,
        TokenType::ARROW                => Terminal::Arrow,
        TokenType::RT_ARROW             => Terminal::RtArrow,

        // ── Operadores de un carácter ─────────
        TokenType::OP_ASSIGN   => Terminal::Assign,
        TokenType::OP_LESS     => Terminal::Less,
        TokenType::OP_GREATER  => Terminal::Greater,
        TokenType::OP_PLUS     => Terminal::Plus,
        TokenType::OP_MINUS    => Terminal::Minus,
        TokenType::OP_MULTIPLY => Terminal::Multiply,
        TokenType::OP_DIVIDE   => Terminal::Divide,
        TokenType::OP_MODULE   => Terminal::Modulo,
        TokenType::OP_POWER_CARET => Terminal::PowerCaret,
        TokenType::OP_AND      => Terminal::And,
        TokenType::OP_OR       => Terminal::Or,
        TokenType::OP_NOT      => Terminal::Not,
        TokenType::OP_CONCAT   => Terminal::Concat,

        // ── Símbolos ──────────────────────────
        TokenType::SEMICOLON => Terminal::Semicolon,
        TokenType::COLON     => Terminal::Colon,
        TokenType::COMMA     => Terminal::Comma,
        TokenType::DOT       => Terminal::Dot,
        TokenType::LPAREN    => Terminal::LParen,
        TokenType::RPAREN    => Terminal::RParen,
        TokenType::LBRACE    => Terminal::LBrace,
        TokenType::RBRACE    => Terminal::RBrace,
        TokenType::LBRACKET  => Terminal::LBracket,
        TokenType::RBRACKET  => Terminal::RBracket,

        // ── Identificador ─────────────────────
        TokenType::IDENTIFIER => Terminal::Identifier,
    };
    Some(terminal)
}

// ─────────────────────────────────────────────
//  TokenStream
//  Wrapper sobre el iterador de tokens del lexer
//  que ya filtra los skippables y traduce.
//  El parser llama a `.next()` y recibe
//  directamente (Terminal, lexeme, line, col).
// ─────────────────────────────────────────────
use crate::lexer::token::Token;

pub struct TokenStream<I: Iterator<Item = Token>> {
    inner: I,
}

impl<I: Iterator<Item = Token>> TokenStream<I> {
    pub fn new(iter: I) -> Self {
        Self { inner: iter }
    }
}

/// Lo que el parser consume en cada paso.
#[derive(Debug, Clone)]
pub struct ParserToken {
    pub terminal: Terminal,
    pub lexeme:   String,
    pub line:     usize,
    pub column:   usize,
}

impl<I: Iterator<Item = Token>> Iterator for TokenStream<I> {
    type Item = ParserToken;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let tok = self.inner.next()?;

            // Saltar whitespace/comentarios/newlines marcados por el lexer
            if tok.skippable {
                continue;
            }

            match token_to_terminal(&tok.token_type) {
                // Token ignorable que no es skippable (ej. ERROR)
                None => continue,

                Some(terminal) => {
                    return Some(ParserToken {
                        terminal,
                        lexeme: tok.lexeme,
                        line:   tok.line,
                        column: tok.column,
                    });
                }
            }
        }
    }
}

// ─────────────────────────────────────────────
//  Tests
// ─────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn make_token(tt: TokenType, lexeme: &str, skippable: bool) -> Token {
        Token::new(tt, lexeme.to_string(), 1, 1, skippable)
    }

    #[test]
    fn whitespace_returns_none() {
        assert!(token_to_terminal(&TokenType::WHITESPACE).is_none());
        assert!(token_to_terminal(&TokenType::COMMENT).is_none());
        assert!(token_to_terminal(&TokenType::NEWLINE).is_none());
    }

    #[test]
    fn keywords_translate_correctly() {
        assert_eq!(token_to_terminal(&TokenType::KW_IF),    Some(Terminal::If));
        assert_eq!(token_to_terminal(&TokenType::KW_WHILE), Some(Terminal::While));
        assert_eq!(token_to_terminal(&TokenType::KW_LET),   Some(Terminal::Let));
        assert_eq!(token_to_terminal(&TokenType::KW_TYPE),  Some(Terminal::Type));
    }

    #[test]
    fn operators_translate_correctly() {
        assert_eq!(token_to_terminal(&TokenType::OP_PLUS),           Some(Terminal::Plus));
        assert_eq!(token_to_terminal(&TokenType::OP_DESTRUCT_ASSIGN), Some(Terminal::DestructAssign));
        assert_eq!(token_to_terminal(&TokenType::OP_EQUAL),          Some(Terminal::Equal));
        assert_eq!(token_to_terminal(&TokenType::ARROW),             Some(Terminal::Arrow));
    }

    #[test]
    fn token_stream_skips_whitespace() {
        let tokens = vec![
            make_token(TokenType::KW_LET,    "let", false),
            make_token(TokenType::WHITESPACE, " ",  true),
            make_token(TokenType::IDENTIFIER, "x",  false),
            make_token(TokenType::EOF,        "",   false),
        ];

        let mut stream = TokenStream::new(tokens.into_iter());

        let t1 = stream.next().unwrap();
        assert_eq!(t1.terminal, Terminal::Let);
        assert_eq!(t1.lexeme, "let");

        let t2 = stream.next().unwrap();
        assert_eq!(t2.terminal, Terminal::Identifier);
        assert_eq!(t2.lexeme, "x");

        let t3 = stream.next().unwrap();
        assert_eq!(t3.terminal, Terminal::Eof);

        assert!(stream.next().is_none());
    }

    #[test]
    fn token_stream_skips_errors() {
        let tokens = vec![
            make_token(TokenType::ERROR,      "€",  false),
            make_token(TokenType::IDENTIFIER, "ok", false),
        ];
        let mut stream = TokenStream::new(tokens.into_iter());
        let t = stream.next().unwrap();
        assert_eq!(t.terminal, Terminal::Identifier);
    }
}