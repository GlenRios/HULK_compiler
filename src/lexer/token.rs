#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TokenType {
    // ---------------------------
    // Especiales
    // ---------------------------
    COMMENT,
    WHITESPACE,
    NEWLINE,
    EOF,
    ERROR,

    // ---------------------------
    // Literales
    // ---------------------------
    STRING,
    CHAR,
    T_NULL,
    NUMBER,

    // ---------------------------
    // Keywords
    // ---------------------------
    KW_IF,
    KW_ELIF,
    KW_ELSE,
    KW_WHILE,
    KW_FOR,
    KW_FUNCTION,
    KW_LET,
    KW_IN,
    KW_TYPE,
    KW_NEW,
    KW_INHERITS,
    KW_BASE,
    KW_PROTOCOL,
    KW_EXTENDS,

    TRUE,
    FALSE,

    // ---------------------------
    // Word operators
    // ---------------------------
    OP_IS,
    OP_AS,

    // ---------------------------
    // Multi-character operators
    // ---------------------------
    OP_POWER_DOUBLE_STAR, // **
    OP_INCREMENT,         // ++
    OP_DECREMENT,         // --
    OP_DOBLE_CONCAT,      // @@
    OP_DESTRUCT_ASSIGN,   // :=
    OP_PLUS_ASSIGN,       // +=
    OP_MINUS_ASSIGN,      // -=
    OP_MULT_ASSIGN,       // *=
    OP_DIV_ASSIGN,        // /=
    OP_MOD_ASSIGN,        // %=
    OP_EQUAL,             // ==
    OP_NOT_EQUAL,         // !=
    OP_LESS_EQ,           // <=
    OP_GREATER_EQ,        // >=
    ARROW,                // =>
    RT_ARROW,             // ->

    // ---------------------------
    // Single-character operators
    // ---------------------------
    OP_ASSIGN,      // =
    OP_LESS,        // <
    OP_GREATER,     // >
    OP_PLUS,        // +
    OP_MINUS,       // -
    OP_MULTIPLY,    // *
    OP_DIVIDE,      // /
    OP_MODULE,      // %
    OP_POWER_CARET, // ^
    OP_AND,         // &
    OP_OR,          // |
    OP_NOT,         // !
    OP_CONCAT,      // @

    // ---------------------------
    // Symbols
    // ---------------------------
    SEMICOLON,
    COLON,
    COMMA,
    DOT,
    LPAREN,
    RPAREN,
    LBRACE,
    RBRACE,
    LBRACKET,
    RBRACKET,

    // ---------------------------
    // Identifier
    // ---------------------------
    IDENTIFIER,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub token_type: TokenType,
    pub lexeme: String,
    pub line: usize,
    pub column: usize,
    pub skippable: bool,
}

impl Token {
    pub fn new(
        token_type: TokenType,
        lexeme: String,
        line: usize,
        column: usize,
        skippable: bool,
    ) -> Self {
        Self {
            token_type,
            lexeme,
            line,
            column,
            skippable,
        }
    }
}
