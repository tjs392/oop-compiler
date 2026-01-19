use crate::token::Token;

pub struct Tokenizer {
    text: String,
    current: usize,
    cached: Option<Token>,
}

impl Tokenizer {
    // takes text string
    pub fn new(text: String) -> Self {
        Tokenizer {
            text,
            current: 0,
            cached: None,
        }
    }

    // borrow token ref from cached for peekoing
    pub fn peek(&mut self) -> &Token {
        if self.cached.is_none() {
            self.cached = Some(self.advance_current());
        }
        return self.cached.as_ref().unwrap();
    }

    // take token from cached
    // cached will be None after called
    pub fn next(&mut self) -> Token {
        if let Some(token) = self.cached.take() {
            token
        } else {
            self.advance_current()
        }
    }

    fn advance_current(&mut self) -> Token {
        while self.current < self.text.len() {
            // since rust uses variable width encoding, we can do byte indexing here 
            // for O(1) opetation
            let c = self.text.as_bytes()[self.current] as char;
            if !c.is_whitespace() {
                break;
            }
            self.current += 1;
        }

        if self.current >= self.text.len() {
            return Token::Eof;
        }

        // consume the next character and increment current
        let ch = self.text.as_bytes()[self.current] as char;
        match ch {
            '(' => { self.current += 1; Token::LeftParen }
            ')' => { self.current += 1; Token::RightParen }
            '{' => { self.current += 1; Token::LeftBrace }
            '}' => { self.current += 1; Token::RightBrace }
            ':' => { self.current += 1; Token::Colon }
            '!' => { self.current += 1; Token::Not }
            '@' => { self.current += 1; Token::AtSign }
            '^' => { self.current += 1; Token::Caret }
            '&' => { self.current += 1; Token::Ampersand }
            '.' => { self.current += 1; Token::Dot }
            ',' => { self.current += 1; Token::Comma }
            '[' => { self.current += 1; Token::LeftBracket }
            ']' => { self.current += 1; Token::RightBracket }
                    
            '+' => { self.current += 1; Token::Operator('+') }
            '-' => { self.current += 1; Token::Operator('-') }
            '*' => { self.current += 1; Token::Operator('*') }
            '/' => { self.current += 1; Token::Operator('/') }

            '=' => { self.current += 1; Token::Equals }
            '_' => { self.current += 1; Token::Identifier("_".to_string()) }
            
            // Tokenizing Digits
            _ if ch.is_ascii_digit() => {
                let start = self.current;
                self.current += 1;
                // This while look will allows us to tokenize digits of any length
                while self.current < self.text.len() {
                    let next_ch = self.text.as_bytes()[self.current] as char;
                    if !next_ch.is_ascii_digit() {
                        break;
                    }
                    self.current += 1;
                }
                let num_str = &self.text[start..self.current];
                let value = num_str.parse::<i64>().unwrap();
                Token::Number(value)
            }
            
            // This is going to tokenize keywords and identifiers
            _ if ch.is_alphabetic() => {
                let start = self.current;
                self.current += 1;
                while self.current < self.text.len() {
                    let next_ch = self.text.as_bytes()[self.current] as char;
                    // This is going to allow for keywords and identifiers that start with a letter
                    // And contain letters or number
                    // Like identifer1 or abc123
                    // But not 123abc or 1
                    if !next_ch.is_alphanumeric() {
                        break;
                    }
                    self.current += 1;
                }
                let fragment = &self.text[start..self.current];
                match fragment {
                    "if" => Token::If,
                    "else" => Token::Else,
                    "ifonly" => Token::IfOnly,
                    "while" => Token::While,
                    "return" => Token::Return,
                    "print" => Token::Print,
                    "this" => Token::This,
                    "class" => Token::Class,
                    "fields" => Token::Fields,
                    "method" => Token::Method,
                    "with" => Token::With,
                    "locals" => Token::Locals,
                    "main" => Token::Main,
                    _ => Token::Identifier(fragment.to_string()),
                }
            }
            
            _ => panic!("Unsupported character: {}", ch),
        }
    }
}

/*
Testing
*/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols() {
        let mut tok = Tokenizer::new("( ) { } ^ & @ ! . : ,".to_string());
        assert!(matches!(tok.next(), Token::LeftParen));
        assert!(matches!(tok.next(), Token::RightParen));
        assert!(matches!(tok.next(), Token::LeftBrace));
        assert!(matches!(tok.next(), Token::RightBrace));
        assert!(matches!(tok.next(), Token::Caret));
        assert!(matches!(tok.next(), Token::Ampersand));
        assert!(matches!(tok.next(), Token::AtSign));
        assert!(matches!(tok.next(), Token::Not));
        assert!(matches!(tok.next(), Token::Dot));
        assert!(matches!(tok.next(), Token::Colon));
        assert!(matches!(tok.next(), Token::Comma));
    }

    #[test]
    fn operators() {
        let mut tok = Tokenizer::new("+ - * /".to_string());
        assert!(matches!(tok.next(), Token::Operator('+')));
        assert!(matches!(tok.next(), Token::Operator('-')));
        assert!(matches!(tok.next(), Token::Operator('*')));
        assert!(matches!(tok.next(), Token::Operator('/')));
    }

    #[test]
    fn keywords() {
        let mut tok = Tokenizer::new("if ifonly while return print this".to_string());
        assert!(matches!(tok.next(), Token::If));
        assert!(matches!(tok.next(), Token::IfOnly));
        assert!(matches!(tok.next(), Token::While));
        assert!(matches!(tok.next(), Token::Return));
        assert!(matches!(tok.next(), Token::Print));
        assert!(matches!(tok.next(), Token::This));
    }

    #[test]
    fn numbers() {
        let mut tok = Tokenizer::new("0 69 2147483647".to_string());
        assert!(matches!(tok.next(), Token::Number(0)));
        assert!(matches!(tok.next(), Token::Number(69)));
        assert!(matches!(tok.next(), Token::Number(2147483647)));
    }

    #[test]
    fn identifiers() {
        let mut tok = Tokenizer::new("x testvar var123 teijisVar TEIJI".to_string());
        match tok.next() {
            Token::Identifier(name) => assert_eq!(name, "x"),
            _ => panic!("Expected Identifier(x)"),
        }
        match tok.next() {
            Token::Identifier(name) => assert_eq!(name, "testvar"),
            _ => panic!("Expected Identifier(testvar)"),
        }
        match tok.next() {
            Token::Identifier(name) => assert_eq!(name, "var123"),
            _ => panic!("Expected Identifier(var123)"),
        }
        match tok.next() {
            Token::Identifier(name) => assert_eq!(name, "teijisVar"),
            _ => panic!("Expected Identifier(teijisVar)"),
        }
        match tok.next() {
            Token::Identifier(name) => assert_eq!(name, "TEIJI"),
            _ => panic!("Expected Identifier(TEIJI)"),
        }
    }

    #[test]
    fn peek_doesnt_consume() {
        let mut tok = Tokenizer::new("62 + 5".to_string());
        assert!(matches!(tok.peek(), Token::Number(62)));
        assert!(matches!(tok.peek(), Token::Number(62)));
        assert!(matches!(tok.next(), Token::Number(62)));
        assert!(matches!(tok.peek(), Token::Operator('+')));
        assert!(matches!(tok.next(), Token::Operator('+')));
    }

    #[test]
    fn whitespace_variants() {
        let mut tok = Tokenizer::new("   69\t+\n5   ".to_string());
        assert!(matches!(tok.next(), Token::Number(69)));
        assert!(matches!(tok.next(), Token::Operator('+')));
        assert!(matches!(tok.next(), Token::Number(5)));
        assert!(matches!(tok.next(), Token::Eof));
    }

    #[test]
    fn no_spaces_between_tokens() {
        let mut tok = Tokenizer::new("62+5".to_string());
        assert!(matches!(tok.next(), Token::Number(62)));
        assert!(matches!(tok.next(), Token::Operator('+')));
        assert!(matches!(tok.next(), Token::Number(5)));
    }

    #[test]
    fn arithmetic() {
        let mut tok = Tokenizer::new("( 3 + 5 )".to_string());
        assert!(matches!(tok.next(), Token::LeftParen));
        assert!(matches!(tok.next(), Token::Number(3)));
        assert!(matches!(tok.next(), Token::Operator('+')));
        assert!(matches!(tok.next(), Token::Number(5)));
        assert!(matches!(tok.next(), Token::RightParen));
    }

    #[test]
    fn field_read() {
        let mut tok = Tokenizer::new("&this.x".to_string());
        assert!(matches!(tok.next(), Token::Ampersand));
        assert!(matches!(tok.next(), Token::This));
        assert!(matches!(tok.next(), Token::Dot));
        match tok.next() {
            Token::Identifier(name) => assert_eq!(name, "x"),
            _ => panic!("Expected Identifier(x)"),
        }
    }

    #[test]
    fn method_call() {
        let mut tok = Tokenizer::new("^x.push(69, y)".to_string());
        assert!(matches!(tok.next(), Token::Caret));
        match tok.next() {
            Token::Identifier(name) => assert_eq!(name, "x"),
            _ => panic!("Expected Identifier(x)"),
        }
        assert!(matches!(tok.next(), Token::Dot));
        match tok.next() {
            Token::Identifier(name) => assert_eq!(name, "push"),
            _ => panic!("Expected Identifier(push)"),
        }
        assert!(matches!(tok.next(), Token::LeftParen));
        assert!(matches!(tok.next(), Token::Number(69)));
        assert!(matches!(tok.next(), Token::Comma));
        match tok.next() {
            Token::Identifier(name) => assert_eq!(name, "y"),
            _ => panic!("Expected Identifier(y)"),
        }
        assert!(matches!(tok.next(), Token::RightParen));
    }

    #[test]
    fn class_reference() {
        let mut tok = Tokenizer::new("@Class".to_string());
        assert!(matches!(tok.next(), Token::AtSign));
        match tok.next() {
            Token::Identifier(name) => assert_eq!(name, "Class"),
            _ => panic!("Expected Identifier(Class)"),
        }
    }
    
    #[test]
    #[should_panic(expected = "Unsupported character")]
    fn invalid_character() {
        let mut tok = Tokenizer::new("#".to_string());
        tok.next();
    }
}






