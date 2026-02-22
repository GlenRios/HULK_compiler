#[derive(Debug, Clone, PartialEq)]
pub enum RegexAST {
    Literal(char),

    Concat(Box<RegexAST>, Box<RegexAST>),
    Union(Box<RegexAST>, Box<RegexAST>),

    Star(Box<RegexAST>),
    Plus(Box<RegexAST>),
    Optional(Box<RegexAST>),

    Dot,

    Range(char, char),
}
