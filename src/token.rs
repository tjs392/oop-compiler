#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    // Top Level
    Class,
    Fields,
    Method,
    Locals,
    Main,
    With,

    // Smybols
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Caret,
    Ampersand,
    AtSign,
    Not,
    Dot,
    Colon,
    Comma,
    LeftBracket,
    RightBracket,

    // Keywords
    This,
    If,
    Else,
    IfOnly,
    While,
    Return,
    Print,
    Eof,
    Operator,
    Number,
    Identifier,
    Equals
}

#[derive(Debug, Clone)]
pub enum Token {
    Class,
    Fields,
    Method,
    Locals,
    Main,
    With,

    // number accept i64 num of course
    Number(i64),
    // operator will accept an operator character like + - / *
    Operator(char),

    Identifier(String),

    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Caret,
    Ampersand,
    AtSign,
    Not,
    Dot,
    LeftBracket,
    RightBracket,

    If,
    Else,
    IfOnly,
    While,
    Return,
    Print,
    Colon,
    Comma,
    Eof,
    This,
    Equals
}

impl Token {
    pub fn get_type(&self) -> TokenType {
        match self {
            Token::Number(_) => TokenType::Number,
            Token::Operator(_) => TokenType::Operator,
            Token::Identifier(_) => TokenType::Identifier,
            Token::LeftParen => TokenType::LeftParen,
            Token::RightParen => TokenType::RightParen,
            Token::LeftBrace => TokenType::LeftBrace,
            Token::RightBrace => TokenType::RightBrace,
            Token::Caret => TokenType::Caret,
            Token::Ampersand => TokenType::Ampersand,
            Token::AtSign => TokenType::AtSign,
            Token::Not => TokenType::Not,
            Token::Dot => TokenType::Dot,
            Token::If => TokenType::If,
            Token::Else => TokenType::Else,
            Token::IfOnly => TokenType::IfOnly,
            Token::While => TokenType::While,
            Token::Return => TokenType::Return,
            Token::Print => TokenType::Print,
            Token::Colon => TokenType::Colon,
            Token::Comma => TokenType::Comma,
            Token::Eof => TokenType::Eof,
            Token::This => TokenType::This,
            Token::Equals => TokenType::Equals,
            Token::Class => TokenType::Class,
            Token::Fields => TokenType::Fields,
            Token::Method => TokenType::Method,
            Token::With => TokenType::With,
            Token::Locals => TokenType::Locals,
            Token::Main => TokenType::Main,
            Token::LeftBracket => TokenType::LeftBracket,
            Token::RightBracket => TokenType::RightBracket,
        }
    }
}