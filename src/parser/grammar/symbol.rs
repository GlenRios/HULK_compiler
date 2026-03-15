use std::fmt;

// ─────────────────────────────────────────────
//  Terminal
//  Cada variante corresponde 1:1 con un TokenType
//  del lexer. Usamos nuestro propio enum para no
//  acoplar el parser directamente al crate lexer.
// ─────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Terminal {
    // Literales
    Number,
    String,
    Char,
    True,
    False,
    Null,

    // Keywords
    If,
    Elif,
    Else,
    While,
    For,
    Function,
    Let,
    In,
    Type,
    New,
    Inherits,
    Base,
    Protocol,
    Extends,

    // Operadores de palabra
    Is,
    As,

    // Operadores multicarácter
    PowerStar,      // **
    Increment,      // ++
    Decrement,      // --
    DoubleConcat,   // @@
    DestructAssign, // :=
    PlusAssign,     // +=
    MinusAssign,    // -=
    MultAssign,     // *=
    DivAssign,      // /=
    ModAssign,      // %=
    Equal,          // ==
    NotEqual,       // !=
    LessEq,         // <=
    GreaterEq,      // >=
    Arrow,          // =>
    RtArrow,        // ->

    // Operadores de un carácter
    Assign,         // =
    Less,           // <
    Greater,        // >
    Plus,           // +
    Minus,          // -
    Multiply,       // *
    Divide,         // /
    Modulo,         // %
    PowerCaret,     // ^
    And,            // &
    Or,             // |
    Not,            // !
    Concat,         // @

    // Símbolos
    Semicolon,      // ;
    Colon,          // :
    Comma,          // ,
    Dot,            // .
    LParen,         // (
    RParen,         // )
    LBrace,         // {
    RBrace,         // }
    LBracket,       // [
    RBracket,       // ]

    // Identificador
    Identifier,

    // Fin de input — símbolo $ en la tabla LALR
    Eof,
}

// ─────────────────────────────────────────────
//  NonTerminal
//  Todos los símbolos no-terminales de la gramática
//  HULK. Se agrupan por área semántica.
// ─────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NonTerminal {
    // Raíz
    Program,

    // Declaraciones de nivel superior
    DeclList,
    Decl,
    FuncDecl,
    TypeDecl,
    ProtocolDecl,

    // Parámetros y argumentos
    ParamList,
    ParamListNonEmpty,
    Param,
    ArgList,
    ArgListNonEmpty,

    // Anotaciones de tipo
    TypeAnnotation,      // `: Type`  (opcional en muchos lugares)
    TypeName,            // un identificador que nombra un tipo

    // Cuerpo de tipo
    TypeArgs,            // `(x, y)` en `type Point(x, y)`
    TypeArgList,
    InheritClause,       // `inherits Point(rho * sin(phi), ...)`
    TypeBody,
    TypeMemberList,
    TypeMember,
    AttributeDef,
    MethodDef,

    // Cuerpo de protocolo
    ProtocolBody,
    ProtocolMemberList,
    ProtocolMember,
    MethodSignature,

    // Expresiones — por precedencia, de menor a mayor
    Expr,               // nivel raíz de expresión
    AssignExpr,         // `:=`  y variantes `+=`, `-=`, …
    OrExpr,             // `|`
    AndExpr,            // `&`
    NotExpr,            // `!`
    CompareExpr,        // `==` `!=` `<` `>` `<=` `>=`
    IsAsExpr,           // `is` `as`
    ConcatExpr,         // `@` `@@`
    AddExpr,            // `+` `-`
    MulExpr,            // `*` `/` `%`
    PowerExpr,          // `^` `**`
    UnaryExpr,          // `-` (unario) `!`
    PostfixExpr,        // `++` `--`
    CallOrAccess,       // llamadas y acceso a miembros
    PrimaryExpr,        // átomo — literal, identificador, `(expr)`

    // Expresiones compuestas
    BlockExpr,          // `{ stmt; stmt; expr }`
    LetExpr,            // `let x = e in e`
    LetBindingList,
    LetBinding,
    IfExpr,             // `if (c) e elif (c) e else e`
    ElifChain,
    WhileExpr,          // `while (c) e`
    ForExpr,            // `for (x in e) e`
    NewExpr,            // `new Type(args)`
    IndexExpr,          // `e[e]`

    // Listas de expresiones
    ExprList,           // dentro de `{ e; e; e }`
    ExprListTail,

    // Vectores
    VectorLiteral,      // `[e1, e2]` o `[expr | id in expr]`

    // Símbolo inicial aumentado (S' → Program $)
    Start,
}

// ─────────────────────────────────────────────
//  Symbol — unión de terminal y no-terminal
//  Es lo que se pone en el lado derecho de
//  una producción o en los stacks del parser.
// ─────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Symbol {
    T(Terminal),
    NT(NonTerminal),
}

impl Symbol {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Symbol::T(_))
    }

    pub fn is_nonterminal(&self) -> bool {
        matches!(self, Symbol::NT(_))
    }
}

// ─────────────────────────────────────────────
//  Display — útil para depuración de la tabla
// ─────────────────────────────────────────────
impl fmt::Display for Terminal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Terminal::Number        => "NUMBER",
            Terminal::String        => "STRING",
            Terminal::Char          => "CHAR",
            Terminal::True          => "true",
            Terminal::False         => "false",
            Terminal::Null          => "null",
            Terminal::If            => "if",
            Terminal::Elif          => "elif",
            Terminal::Else          => "else",
            Terminal::While         => "while",
            Terminal::For           => "for",
            Terminal::Function      => "function",
            Terminal::Let           => "let",
            Terminal::In            => "in",
            Terminal::Type          => "type",
            Terminal::New           => "new",
            Terminal::Inherits      => "inherits",
            Terminal::Base          => "base",
            Terminal::Protocol      => "protocol",
            Terminal::Extends       => "extends",
            Terminal::Is            => "is",
            Terminal::As            => "as",
            Terminal::PowerStar     => "**",
            Terminal::Increment     => "++",
            Terminal::Decrement     => "--",
            Terminal::DoubleConcat  => "@@",
            Terminal::DestructAssign => ":=",
            Terminal::PlusAssign    => "+=",
            Terminal::MinusAssign   => "-=",
            Terminal::MultAssign    => "*=",
            Terminal::DivAssign     => "/=",
            Terminal::ModAssign     => "%=",
            Terminal::Equal         => "==",
            Terminal::NotEqual      => "!=",
            Terminal::LessEq        => "<=",
            Terminal::GreaterEq     => ">=",
            Terminal::Arrow         => "=>",
            Terminal::RtArrow       => "->",
            Terminal::Assign        => "=",
            Terminal::Less          => "<",
            Terminal::Greater       => ">",
            Terminal::Plus          => "+",
            Terminal::Minus         => "-",
            Terminal::Multiply      => "*",
            Terminal::Divide        => "/",
            Terminal::Modulo        => "%",
            Terminal::PowerCaret    => "^",
            Terminal::And           => "&",
            Terminal::Or            => "|",
            Terminal::Not           => "!",
            Terminal::Concat        => "@",
            Terminal::Semicolon     => ";",
            Terminal::Colon         => ":",
            Terminal::Comma         => ",",
            Terminal::Dot           => ".",
            Terminal::LParen        => "(",
            Terminal::RParen        => ")",
            Terminal::LBrace        => "{",
            Terminal::RBrace        => "}",
            Terminal::LBracket      => "[",
            Terminal::RBracket      => "]",
            Terminal::Identifier    => "IDENTIFIER",
            Terminal::Eof           => "$",
        };
        write!(f, "{}", s)
    }
}

impl fmt::Display for NonTerminal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Symbol::T(t)  => write!(f, "{}", t),
            Symbol::NT(n) => write!(f, "{}", n),
        }
    }
}