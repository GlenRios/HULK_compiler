use crate::lexer::token::TokenType;

pub struct TokenDefinition {
    pub token_type: TokenType,
    pub regex: &'static str,
    pub skippable: bool,
}

impl TokenDefinition {
    pub fn default_token_definitions() -> Vec<TokenDefinition> {
        vec![
            // -----------------------------------
            // Comentarios y espacios
            // -----------------------------------
            TokenDefinition {
                token_type: TokenType::COMMENT,
                regex: "// *([ -\t]|[\x0B-~])*",
                skippable: true,
            },
            TokenDefinition {
                token_type: TokenType::COMMENT,
                regex: "/[*].*[*]/",
                skippable: true,
            },
            TokenDefinition {
                token_type: TokenType::WHITESPACE,
                regex: "[ \t]+",
                skippable: true,
            },
            TokenDefinition {
                token_type: TokenType::NEWLINE,
                regex: "[\n]+",
                skippable: true,
            },
            // -----------------------------------
            // Literales
            // -----------------------------------
            TokenDefinition {
                token_type: TokenType::STRING,
                regex: "\"[ -~]*\"",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::CHAR,
                regex: "'.'",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::T_NULL,
                regex: "Null",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::NUMBER,
                regex: "[0-9]*([.][0-9]+)?",
                skippable: false,
            },
            // -----------------------------------
            // Keywords
            // -----------------------------------
            TokenDefinition {
                token_type: TokenType::KW_IF,
                regex: "if",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::KW_ELIF,
                regex: "elif",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::KW_ELSE,
                regex: "else",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::KW_WHILE,
                regex: "while",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::KW_FOR,
                regex: "for",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::KW_FUNCTION,
                regex: "function",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::KW_LET,
                regex: "let",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::KW_IN,
                regex: "in",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::KW_TYPE,
                regex: "type",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::KW_NEW,
                regex: "new",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::KW_INHERITS,
                regex: "inherits",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::KW_BASE,
                regex: "base",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::KW_PROTOCOL,
                regex: "protocol",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::KW_EXTENDS,
                regex: "extends",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::TRUE,
                regex: "true",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::FALSE,
                regex: "false",
                skippable: false,
            },
            // -----------------------------------
            // Word operators
            // -----------------------------------
            TokenDefinition {
                token_type: TokenType::OP_IS,
                regex: "is",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_AS,
                regex: "as",
                skippable: false,
            },
            // -----------------------------------
            // Multi-character operators (orden importa)
            // -----------------------------------
            TokenDefinition {
                token_type: TokenType::OP_POWER_DOUBLE_STAR,
                regex: "[*][*]",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_INCREMENT,
                regex: "[+][+]",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_DECREMENT,
                regex: "--",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_DOBLE_CONCAT,
                regex: "@@",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_DESTRUCT_ASSIGN,
                regex: ":=",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_PLUS_ASSIGN,
                regex: "[+]=",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_MINUS_ASSIGN,
                regex: "-=",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_MULT_ASSIGN,
                regex: "[*]=",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_DIV_ASSIGN,
                regex: "/=",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_MOD_ASSIGN,
                regex: "%=",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_EQUAL,
                regex: "==",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_NOT_EQUAL,
                regex: "!=",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_LESS_EQ,
                regex: "<=",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_GREATER_EQ,
                regex: ">=",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::ARROW,
                regex: "=>",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::RT_ARROW,
                regex: "->",
                skippable: false,
            },
            // -----------------------------------
            // Single-character operators
            // -----------------------------------
            TokenDefinition {
                token_type: TokenType::OP_ASSIGN,
                regex: "=",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_LESS,
                regex: "<",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_GREATER,
                regex: ">",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_PLUS,
                regex: "[+]",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_MINUS,
                regex: "-",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_MULTIPLY,
                regex: "[*]",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_DIVIDE,
                regex: "/",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_MODULE,
                regex: "%",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_POWER_CARET,
                regex: "^",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_AND,
                regex: "&",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_OR,
                regex: "[|]",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_NOT,
                regex: "!",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::OP_CONCAT,
                regex: "@",
                skippable: false,
            },
            // -----------------------------------
            // Symbols
            // -----------------------------------
            TokenDefinition {
                token_type: TokenType::SEMICOLON,
                regex: ";",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::COLON,
                regex: ":",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::COMMA,
                regex: ",",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::DOT,
                regex: "[.]",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::LPAREN,
                regex: "[(]",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::RPAREN,
                regex: ")",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::LBRACE,
                regex: "{",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::RBRACE,
                regex: "}",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::LBRACKET,
                regex: "[[]",
                skippable: false,
            },
            TokenDefinition {
                token_type: TokenType::RBRACKET,
                regex: "]",
                skippable: false,
            },
            // -----------------------------------
            // IDENTIFIER (SIEMPRE EL ÚLTIMO)
            // -----------------------------------
            TokenDefinition {
                token_type: TokenType::IDENTIFIER,
                regex: "[a-zA-Z]+[a-zA-Z0-9_]*",
                skippable: false,
            },
        ]
    }
}
