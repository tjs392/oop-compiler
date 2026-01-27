use crate::token::{Token, Operator};

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
            
            if c.is_whitespace() {
                self.current += 1;
                continue;
            }

            if c == '#' {
                while self.current < self.text.len() {
                    let ch = self.text.as_bytes()[self.current] as char;
                    self.current += 1;
                    if ch == '\n' {
                        break;
                    }
                }
                continue;
            }

            break;
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
                    
            '+' => { self.current += 1; Token::Operator(Operator::Plus) }
            '-' => { self.current += 1; Token::Operator(Operator::Minus) }
            '*' => { self.current += 1; Token::Operator(Operator::Multiply) }
            '/' => { self.current += 1; Token::Operator(Operator::Divide) }
            '<' => { self.current += 1; Token::Operator(Operator::LessThan) }
            '>' => { self.current += 1; Token::Operator(Operator::GreaterThan) }
            '|' => { self.current += 1; Token::Operator(Operator::BitwiseOr) }

            '=' => {
                self.current += 1;
                if self.current < self.text.len() {
                    let next_ch = self.text.as_bytes()[self.current] as char;
                    if next_ch == '=' {
                        self.current += 1;
                        return Token::Operator(Operator::Equals);
                    }
                }
                Token::Equals
            }

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
