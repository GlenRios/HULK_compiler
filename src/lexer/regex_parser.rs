use crate::lexer::regex_ast::RegexAST;

pub struct RegexParser {
    input: Vec<char>,
    pos: usize,
}

impl RegexParser {
    pub fn new(pattern: &str) -> Self {
        Self {
            input: pattern.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).cloned()
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn consume(&mut self) -> Option<char> {
        let ch = self.peek();
        self.advance();
        ch
    }

    pub fn parse(&mut self) -> RegexAST {
        self.parse_union()
    }

    fn parse_union(&mut self) -> RegexAST {
        let mut node = self.parse_concat();

        while let Some('|') = self.peek() {
            self.advance();
            let right = self.parse_concat();
            node = RegexAST::Union(Box::new(node), Box::new(right));
        }

        node
    }

    fn parse_concat(&mut self) -> RegexAST {
        let mut node = self.parse_quantifier();

        while let Some(ch) = self.peek() {
            if ch == ')' || ch == '|' {
                break;
            }

            let right = self.parse_quantifier();
            node = RegexAST::Concat(Box::new(node), Box::new(right));
        }

        node
    }

    fn parse_quantifier(&mut self) -> RegexAST {
        let mut node = self.parse_primary();

        loop {
            match self.peek() {
                Some('*') => {
                    self.advance();
                    node = RegexAST::Star(Box::new(node));
                }
                Some('+') => {
                    self.advance();
                    node = RegexAST::Plus(Box::new(node));
                }
                Some('?') => {
                    self.advance();
                    node = RegexAST::Optional(Box::new(node));
                }
                _ => break,
            }
        }

        node
    }

    fn parse_primary(&mut self) -> RegexAST {
        match self.peek() {
            Some('(') => {
                self.advance();
                let node = self.parse_union();
                self.advance(); // consume ')'
                node
            }

            Some('.') => {
                self.advance();
                RegexAST::Dot
            }

            Some('[') => self.parse_char_class(),

            Some(ch) => {
                self.advance();
                RegexAST::Literal(ch)
            }

            None => panic!("Unexpected end of regex"),
        }
    }

    fn parse_char_class(&mut self) -> RegexAST {
        self.advance(); // consumir '['

        let mut elements = Vec::new();

        while let Some(c) = self.peek() {
            if c == ']' {
                break;
            }

            let start = self.consume().unwrap();

            if self.peek() == Some('-') {
                self.advance(); // consumir '-'

                let end = self.consume().expect("Expected end of range");

                elements.push(RegexAST::Range(start, end));
            } else {
                elements.push(RegexAST::Literal(start));
            }
        }

        if self.peek() != Some(']') {
            panic!("Unclosed character class");
        }

        self.advance(); // consumir ']'

        if elements.is_empty() {
            panic!("Empty character class");
        }

        let mut iter = elements.into_iter();
        let mut ast = iter.next().unwrap();

        for element in iter {
            ast = RegexAST::Union(Box::new(ast), Box::new(element));
        }

        ast
    }
}

///////////////////  TESTS  ////////////////
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::regex_ast::RegexAST;

    fn lit(c: char) -> RegexAST {
        RegexAST::Literal(c)
    }

    #[test]
    fn precedence_union_vs_concat() {
        // a|bc  => a | (b c)
        let mut parser = RegexParser::new("a|bc");

        assert_eq!(
            parser.parse(),
            RegexAST::Union(
                Box::new(lit('a')),
                Box::new(RegexAST::Concat(Box::new(lit('b')), Box::new(lit('c'))))
            )
        );
    }

    #[test]
    fn grouping_changes_precedence() {
        // (a|b)c => (a|b) c
        let mut parser = RegexParser::new("(a|b)c");

        assert_eq!(
            parser.parse(),
            RegexAST::Concat(
                Box::new(RegexAST::Union(Box::new(lit('a')), Box::new(lit('b')))),
                Box::new(lit('c'))
            )
        );
    }

    #[test]
    fn star_operator() {
        let mut parser = RegexParser::new("a*");

        assert_eq!(parser.parse(), RegexAST::Star(Box::new(lit('a'))));
    }

    #[test]
    fn plus_operator() {
        let mut parser = RegexParser::new("a+");

        assert_eq!(parser.parse(), RegexAST::Plus(Box::new(lit('a'))));
    }

    #[test]
    fn optional_operator() {
        let mut parser = RegexParser::new("a?");

        assert_eq!(parser.parse(), RegexAST::Optional(Box::new(lit('a'))));
    }

    #[test]
    fn chained_quantifiers() {
        // a*+
        let mut parser = RegexParser::new("a*+");

        assert_eq!(
            parser.parse(),
            RegexAST::Plus(Box::new(RegexAST::Star(Box::new(lit('a')))))
        );
    }

    #[test]
    fn complex_expression() {
        // (a|b)*c+
        let mut parser = RegexParser::new("(a|b)*c+");

        assert_eq!(
            parser.parse(),
            RegexAST::Concat(
                Box::new(RegexAST::Star(Box::new(RegexAST::Union(
                    Box::new(lit('a')),
                    Box::new(lit('b'))
                )))),
                Box::new(RegexAST::Plus(Box::new(lit('c'))))
            )
        );
    }

    #[test]
    fn range_expression() {
        let mut parser = RegexParser::new("[.]");

        assert_eq!(parser.parse(), RegexAST::Range('.', '.'));
    }

    #[test]
    fn dot_operator() {
        let mut parser = RegexParser::new(".");

        assert_eq!(parser.parse(), RegexAST::Dot);
    }

    #[test]
    fn very_complex_expression() {
        // a(b|c)*d?
        let mut parser = RegexParser::new("a(b|c)*d?");

        assert_eq!(
            parser.parse(),
            RegexAST::Concat(
                Box::new(RegexAST::Concat(
                    Box::new(lit('a')),
                    Box::new(RegexAST::Star(Box::new(RegexAST::Union(
                        Box::new(lit('b')),
                        Box::new(lit('c'))
                    ))))
                )),
                Box::new(RegexAST::Optional(Box::new(lit('d'))))
            )
        );
    }
}
