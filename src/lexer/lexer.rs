use crate::lexer::master_nfa::MasterNFA;
use crate::lexer::token::{Token, TokenType};
use crate::lexer::token_definition::TokenDefinition;

pub struct Lexer {
    input: Vec<char>,
    position: usize,
    line: usize,
    column: usize,
    master: MasterNFA,
}
impl Lexer {
    pub fn new(input: &str, master: MasterNFA) -> Self {
        Self {
            input: input.chars().collect(),
            position: 0,
            line: 1,
            column: 1,
            master,
        }
    }
    fn advance(&mut self, length: usize) {
        for _ in 0..length {
            if self.input[self.position] == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
            self.position += 1;
        }
    }
    pub fn next_token(&mut self) -> Token {
        if self.position >= self.input.len() {
            return Token::new(TokenType::EOF, "".into(), self.line, self.column, false);
        }

        let start_pos = self.position;
        let start_line = self.line;
        let start_column = self.column;

        if let Some((token_type, length, skippable)) =
            self.master.match_longest(&self.input, self.position)
        {
            let lexeme: String = self.input[start_pos..start_pos + length].iter().collect();

            self.advance(length);

            return Token::new(token_type, lexeme, start_line, start_column, skippable);
        }

        // Error léxico
        let bad_char = self.input[self.position];
        self.advance(1);

        Token::new(
            TokenType::ERROR,
            bad_char.to_string(),
            start_line,
            start_column,
            false,
        )
    }
    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();

        loop {
            let token = self.next_token();

            if token.token_type == TokenType::EOF {
                tokens.push(token);
                break;
            }

            if !token.skippable {
                tokens.push(token);
            }
        }

        tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_single_char() {
        // let defs = TokenDefinition::default_token_definitions();
        // let master = MasterNFA::from_token_definitions(&defs);

        // println!("{:?}", master);
        let defs = vec![TokenDefinition {
            token_type: TokenType::OP_PLUS,
            regex: r"[+]",
            skippable: false,
        }];

        let master = MasterNFA::from_token_definitions(&defs);
        let mut lexer = Lexer::new("+", master);
        let tokens = lexer.tokenize();

        println!("{:?}", tokens);
    }

    #[test]
    fn test_single_plus() {
        let defs = TokenDefinition::default_token_definitions();
        let master = MasterNFA::from_token_definitions(&defs);

        let mut lexer = Lexer::new("+", master);
        let tokens = lexer.tokenize();

        assert_eq!(tokens[0].token_type, TokenType::OP_PLUS);
        assert_eq!(tokens[1].token_type, TokenType::EOF);
    }
    #[test]
    fn test_integer() {
        let defs = TokenDefinition::default_token_definitions();
        let master = MasterNFA::from_token_definitions(&defs);

        let mut lexer = Lexer::new("123", master);
        let tokens = lexer.tokenize();

        assert_eq!(tokens[0].token_type, TokenType::NUMBER);
        assert_eq!(tokens[0].lexeme, "123");
    }
    #[test]
    fn test_identifier() {
        let defs = TokenDefinition::default_token_definitions();
        let master = MasterNFA::from_token_definitions(&defs);

        let mut lexer = Lexer::new("variable123", master);
        let tokens = lexer.tokenize();

        assert_eq!(tokens[0].token_type, TokenType::IDENTIFIER);
    }
    #[test]
    fn test_equal_conflict() {
        let defs = TokenDefinition::default_token_definitions();
        let master = MasterNFA::from_token_definitions(&defs);

        let mut lexer = Lexer::new("==", master);
        let tokens = lexer.tokenize();

        assert_eq!(tokens[0].token_type, TokenType::OP_EQUAL);
    }
    #[test]
    fn test_plus_assign() {
        let defs = TokenDefinition::default_token_definitions();
        let master = MasterNFA::from_token_definitions(&defs);

        let mut lexer = Lexer::new("+=", master);
        let tokens = lexer.tokenize();

        assert_eq!(tokens[0].token_type, TokenType::OP_PLUS_ASSIGN);
    }
    #[test]
    fn test_increment() {
        let defs = TokenDefinition::default_token_definitions();
        let master = MasterNFA::from_token_definitions(&defs);

        let mut lexer = Lexer::new("++", master);
        let tokens = lexer.tokenize();

        assert_eq!(tokens[0].token_type, TokenType::OP_INCREMENT);
    }

    #[test]
    fn test_expression() {
        let defs = TokenDefinition::default_token_definitions();
        let master = MasterNFA::from_token_definitions(&defs);

        let mut lexer = Lexer::new("let x = 10 + 20;", master);
        let tokens = lexer.tokenize();

        let types: Vec<TokenType> = tokens.iter().map(|t| t.token_type.clone()).collect();

        assert_eq!(
            types,
            vec![
                TokenType::KW_LET,
                TokenType::IDENTIFIER,
                TokenType::OP_ASSIGN,
                TokenType::NUMBER,
                TokenType::OP_PLUS,
                TokenType::NUMBER,
                TokenType::SEMICOLON,
                TokenType::EOF
            ]
        );
    }

    #[test]
    fn test_function() {
        let defs = TokenDefinition::default_token_definitions();
        let master = MasterNFA::from_token_definitions(&defs);

        let code = "function add(a,b){ return a+b; }";

        let mut lexer = Lexer::new(code, master);
        let tokens = lexer.tokenize();

        assert!(
            tokens
                .iter()
                .any(|t| t.token_type == TokenType::KW_FUNCTION)
        );
        assert!(tokens.iter().any(|t| t.token_type == TokenType::LPAREN));
        assert!(tokens.iter().any(|t| t.token_type == TokenType::RBRACE));
    }

    #[test]
    fn test_comment_skipped() {
        let defs = TokenDefinition::default_token_definitions();
        let master = MasterNFA::from_token_definitions(&defs);

        let mut lexer = Lexer::new("// hola\nlet", master);
        let tokens = lexer.tokenize();
        println!("{:?}", tokens);

        assert_eq!(tokens[0].token_type, TokenType::KW_LET);
    }

    #[test]
    fn test_unknown_symbol() {
        let defs = TokenDefinition::default_token_definitions();
        let master = MasterNFA::from_token_definitions(&defs);

        let mut lexer = Lexer::new("$", master);
        let tokens = lexer.tokenize();

        assert_eq!(tokens[0].token_type, TokenType::ERROR);
    }

    #[test]
    fn test_decimal_number() {
        let defs = TokenDefinition::default_token_definitions();
        let master = MasterNFA::from_token_definitions(&defs);

        let mut lexer = Lexer::new("3.1416", master);
        let tokens = lexer.tokenize();

        assert_eq!(tokens[0].token_type, TokenType::NUMBER);
    }

    #[test]
    fn debug_large_program() {
        let input = r#"
            // Programa de prueba para stress del lexer

            /* 
            Comentario multilínea
            con símbolos raros: @@@ *** !!! <= >= == !=
            */

            function factorial(n) {
                if (n <= 1) {
                    return 1;
                } else {
                    return n * factorial(n - 1);
                }
            }

            function main() {

                let x = 10;
                let y = 20;
                let letter = "let no es keyword aqui";
                let ifx = 5;

                x += 5;
                y -= 3;
                x *= 2;
                y /= 4;
                x %= 3;

                let power = x ** 2 ^ 3;

                if (x == y || x != y && true) {
                    print("Comparacion verdadera @@");
                }

                x++;
                y--;

                let decimal = 123.456;
                let zero = 0;
                let maybe = 10.0;

                let str = "Simbolos especiales: !@#$%^&*()_+[]{}|;:,.<>?";

                type Person inherits Human {
                    function new(name) {
                        base(name);
                    }
                }

                protocol Drawable extends Renderable {
                }

                for (let i = 0; i < 10; i++) {
                    print(i);
                }

                while (x > 0) {
                    x--;
                }

                let destruct := 5;
                let arrow = (a, b) => a + b;
                let ref = object->method();

                return Null;
            }
            "#;
        let defs = TokenDefinition::default_token_definitions();
        let master = MasterNFA::from_token_definitions(&defs);

        let mut lexer = Lexer::new(input, master);
        let tokens = lexer.tokenize();

        for token in tokens {
            println!("{:?}", token);
        }
    }

    #[test]
    fn debug_keyword_vs_identifier() {
        let input = r#"
                let letx = 5;
                letx = letx + 1;

                ifelse = 10;
                if (ifelse > 0) {
                    print("ok");
                }

                baseball = 3;
                inheritsValue = 4;
                protocolX = 5;
                "#;

        let defs = TokenDefinition::default_token_definitions();
        let master = MasterNFA::from_token_definitions(&defs);

        let mut lexer = Lexer::new(input, master);
        let tokens = lexer.tokenize();

        for token in tokens {
            println!("{:?}", token);
        }
    }
    #[test]
    fn debug_compact_operators() {
        let input = r#"
            let x=10;
            x+=5;
            x=x++ + --x * 2**3;
            if(x<=10&&x!=0||true){
                print("edge");
            }
            "#;
        let defs = TokenDefinition::default_token_definitions();
        let master = MasterNFA::from_token_definitions(&defs);

        let mut lexer = Lexer::new(input, master);
        let tokens = lexer.tokenize();

        for token in tokens {
            println!("{:?}", token);
        }
    }

    #[test]
    fn debug_number_edge_cases() {
        let input = r#"
            let a = .5;
            let b = 5.;
            let c = 0.0;
            let d = 00.1;
            let e = 123.;
            "#;

        let defs = TokenDefinition::default_token_definitions();
        let master = MasterNFA::from_token_definitions(&defs);

        let mut lexer = Lexer::new(input, master);
        let tokens = lexer.tokenize();
        for token in tokens {
            println!("{:?}", token);
        }
    }

    #[test]
    fn debug_comment_stress() {
        let input = r#"
        // comentario simple
        // comentario con simbolos !@#$%^&*()_+
        /*
        multilinea
        con operadores <= >= == !=
        */
        /* anidado? */
        /*
        sin cerrar
        "#;
        let defs = TokenDefinition::default_token_definitions();
        let master = MasterNFA::from_token_definitions(&defs);

        let mut lexer = Lexer::new(input, master);
        let tokens = lexer.tokenize();

        for token in tokens {
            println!("{:?}", token);
        }
    }
}
