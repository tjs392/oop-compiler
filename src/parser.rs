use crate::token::{Token, TokenType};
use crate::tokenizer::Tokenizer;
use crate::expression::Expression;
use crate::statement::Statement;
use crate::ast::{Method, Class, Program};

pub struct Parser {
    tok: Tokenizer,
}

impl Parser {
    pub fn new(tok: Tokenizer) -> Self {
        Parser { tok }
    }

    pub fn parse_expr(&mut self) -> Expression {
        match self.tok.next() {
            Token::Eof => panic!("No expression to parse: EOF"),

            Token::Number(n) => Expression::Constant(n),

            Token::Identifier(name) => Expression::Variable(name),

            Token::LeftParen => {
                let lhs = self.parse_expr();

                let op = match self.tok.next() {
                    Token::Operator(c) => c,
                    other => panic!("Expected operator but found {:?}", other),
                };

                let rhs = self.parse_expr();

                match self.tok.next() {
                    Token::RightParen => {},
                    other => panic!("Expected right parenthesis but found {:?}", other),
                }

                Expression::Binop {
                    lhs: Box::new(lhs),
                    op,
                    rhs: Box::new(rhs),
                }
            }

            Token::Ampersand => {
                // reads &base.fieldname
                let base = self.parse_expr();

                match self.tok.next() {
                    Token::Dot => {},
                    other => panic!("Expected . but found {:?}", other),
                }

                let field_name = match self.tok.next() {
                    Token::Identifier(name) => name,
                    other => panic!("Expected field name but found {:?}", other),
                };

                Expression::FieldRead {
                    base: Box::new(base),
                    field_name,
                }
            }

            Token::Caret => {
                // this is method call
                //^base.method(args1, 2, 3..)
                let base = self.parse_expr();

                match self.tok.next() {
                    Token::Dot => {},
                    other => panic!("Expected . but found {:?}", other),
                }

                let method_name = match self.tok.next() {
                    Token::Identifier(name) => name,
                    other => panic!("Expected valid method name but found {:?}", other),
                };

                match self.tok.next() {
                    Token::LeftParen => {},
                    other => panic!("Expected left paren but found {:?}", other),
                }

                // now parsing arguments to method
                let mut args = Vec::<Expression>::new();
                while self.tok.peek().get_type() != TokenType::RightParen {
                    let arg = self.parse_expr();
                    eprintln!("Parsed arg: {:?}", arg);
                    args.push(arg);
                    
                    if self.tok.peek().get_type() == TokenType::Comma {
                        self.tok.next();
                    }
                }

                self.tok.next();

                Expression::MethodCall {
                    base: Box::new(base),
                    method_name,
                    args,
                }
            }

            Token::AtSign => {
                // this is class refernce
                // @ClassName
                let class_name = match self.tok.next() {
                    Token::Identifier(name) => name,
                    other => panic!("Expected valid class name but found {:?}", other),
                };

                Expression::ClassRef(class_name)
            }

            Token::This => Expression::ThisExpr,

            other => panic!("Token {:?} is not a valid start of an expression", other),

        }
    }

    pub fn parse_statement(&mut self) -> Statement {
        match self.tok.peek() {
            
            // return e
            Token::Return => {
                self.tok.next();
                let expression = self.parse_expr();
                Statement::Return(expression)
            }
            
            // print(e)
            Token::Print => {
                self.tok.next();

                match self.tok.next() {
                    Token::LeftParen => {},
                    other => panic!("Expected ( fter print, got {:?}", other),
                }

                let expr = self.parse_expr();

                match self.tok.next() {
                    Token::RightParen => {},
                    other => panic!("Expected ) after print expression, got {:?}", other),
                }

                Statement::Print(expr)
            }

            // if e: { <newline> <one or more statements> } else { <newline> <one or more statements> }
            Token::If => {
                self.tok.next();
                let condition = self.parse_expr();

                match self.tok.next() {
                    Token::Colon => {},
                    other => panic!("Expected : after if condiiton, got {:?}", other),
                }

                match self.tok.next() {
                    Token::LeftBrace => {},
                    other => panic!("Expected {{ after if:, got {:?}", other),
                }

                let mut then_body = Vec::<Statement>::new();
                while self.tok.peek().get_type() != TokenType::RightBrace {
                    then_body.push(self.parse_statement());
                }
                self.tok.next();

                match self.tok.next() {
                    Token::Else => {},
                    other => panic!("Expected else after if block, got {:?}", other),
                }

                match self.tok.next() {
                    Token::LeftBrace => {},
                    other => panic!("Expected {{ after else, got {:?}", other),
                }

                let mut else_body = Vec::<Statement>::new();
                while self.tok.peek().get_type() != TokenType::RightBrace {
                    else_body.push(self.parse_statement());
                }
                self.tok.next();
                
                Statement::If { condition, then_body, else_body }
            }

            // ifonly e: { <newline> <one or more statements> }
            Token::IfOnly => {
                self.tok.next();
                let condition = self.parse_expr();

                match self.tok.next() {
                    Token::Colon => {},
                    other => panic!("Expected : after if condition, got {:?}", other),
                }

                match self.tok.next() {
                    Token::LeftBrace => {},
                    other => panic!("Expected {{ after if:, got {:?}", other),
                }

                let mut body = Vec::<Statement>::new();
                while self.tok.peek().get_type() != TokenType::RightBrace {
                    body.push(self.parse_statement());
                }
                self.tok.next();
                
                Statement::IfOnly { condition, body }
            }

            // while e: { <newline> <one or more statements> }
            Token::While => {
                self.tok.next();
                let condition = self.parse_expr();
                
                match self.tok.next() {
                    Token::Colon => {},
                    other => panic!("Expected : after while condition , got {:?}", other),
                }
                
                match self.tok.next() {
                    Token::LeftBrace => {},
                    other => panic!("Expected {{ after while:, got {:?}", other),
                }
                
                let mut body = Vec::<Statement>::new();
                while self.tok.peek().get_type() != TokenType::RightBrace {
                    body.push(self.parse_statement());
                }
                self.tok.next();
                
                Statement::While { condition, body }
            }

            // !e.f = e for field update
            Token::Not => {
                self.tok.next();

                let base = self.parse_expr();
                
                match self.tok.next() {
                    Token::Dot => {},
                    other => panic!("Expected . in field write, got {:?}", other),
                }

                let field = match self.tok.next() {
                    Token::Identifier(name) => name,
                    other => panic!("EXpected field name, got {:?}", other),
                };

                match self.tok.next() {
                    Token::Equals => {},
                    other => panic!("Expected = in field write, got {:?}", other),
                }

                let value = self.parse_expr();
                
                Statement::FieldWrite { base, field, value }
            }

            Token::Identifier(name) => {
                let variable_name = name.clone();
                self.tok.next();

                match self.tok.next() {
                    Token::Equals => {},
                    other => panic!("Expected = in field write, got {:?}", other),
                }

                let expression = self.parse_expr();

                if variable_name == "_" {
                    Statement::Discard(expression)
                } else {
                    Statement::Assignment { variable: variable_name, expression }
                }
            }
            
            other => panic!("UNexpected token at start of statement: {:?}", other),
        }
    }

    pub fn parse_method(&mut self) -> Method {
        // method m(a, b, c, ...) with locals q, r, s, ...:
        match self.tok.next() {
            Token::Method => {},
            other => panic!("Expected 'method', got {:?}", other),
        };

        let name = match self.tok.next() {
            Token::Identifier(n) => n,
            other => panic!("Expected method name, got {:?}", other),
        };

        match self.tok.next() {
            Token::LeftParen => {},
            other => panic!("expected '(' after method name, got {:?}", other),
        }

        let mut args = Vec::<String>::new();
        while self.tok.peek().get_type() != TokenType::RightParen {
            match self.tok.next() {
                Token::Identifier(arg) => args.push(arg),
                other => panic!("Expected argument, got {:?}", other)
            }

            if self.tok.peek().get_type() == TokenType::Comma {
                self.tok.next();
            }
        }
        self.tok.next();

        match self.tok.next() {
            Token::With => {},
            other => panic!("Expected 'with' after arguments, got {:?}", other),
        }

        match self.tok.next() {
            Token::Locals => {},
            other => panic!("Expected 'locals' after 'with', got {:?}", other),
        }

        let mut locals = Vec::<String>::new();
        while self.tok.peek().get_type() != TokenType::Colon {
            match self.tok.next() {
                Token::Identifier(local) => locals.push(local),
                other => panic!("Expected local variable name, but got {:?}", other),
            }

            if self.tok.peek().get_type() == TokenType::Comma {
                self.tok.next();
            }
        }
        self.tok.next();

        let mut body = Vec::<Statement>::new();
        loop {
            let peek_type = self.tok.peek().get_type();
            if peek_type == TokenType::Method || peek_type == TokenType::RightBracket {
                break;
            }
            body.push(self.parse_statement());
        }

        Method { name, args, locals, body}
    }

    pub fn parse_class(&mut self) -> Class {
        /*
        class NAME [
            fields x, y, z, ....
            method m(a, b, c, ...) with locals q, r, s, ...:
                <one or more statements>
            method m2(...) with locals ...:
                <one or more statements>
        ]
        */
        match self.tok.next() {
            Token::Class => {},
            other => panic!("Expected 'class', got {:?}", other),

        }

        let name = match self.tok.next() {
            Token::Identifier(name) => name,
            other => panic!("Expected class name, got {:?}", other),
        };

        match self.tok.next() {
            Token::LeftBracket => {},
            other => panic!("Expected '[', got {:?}", other),
        }

        match self.tok.next() {
            Token::Fields => {},
            other => panic!("Expected 'fields, got {:?}", other),
        }

        let mut fields = Vec::<String>::new();
        while self.tok.peek().get_type() != TokenType::Method && self.tok.peek().get_type() != TokenType::RightBracket {
            match self.tok.next() {
                Token::Identifier(field) => fields.push(field),
                other => panic!("Expected a field name, got {:?}", other),
            }

            if self.tok.peek().get_type() == TokenType::Comma {
                self.tok.next();
            }
        }

        let mut methods = Vec::<Method>::new();
        while self.tok.peek().get_type() == TokenType::Method {
            methods.push(self.parse_method());
        }

        match self.tok.next() {
            Token::RightBracket => {},
            other => panic!("Expected ']' at end of class, got {:?}", other),
        }
        
        Class { name, fields, methods }
    }

    pub fn parse_program(&mut self) -> Program {
        let mut classes = Vec::<Class>::new();

        while self.tok.peek().get_type() == TokenType::Class {
            classes.push(self.parse_class());
        }

        match self.tok.next() {
            Token::Main => {},
            other => panic!("Expected 'main' but got {:?}", other),
        }

        match self.tok.next() {
            Token::With => {},
            other => panic!("Expected 'with', but got {:?}", other),
        }

        let mut main_locals = Vec::<String>::new();
        while self.tok.peek().get_type() != TokenType::Colon {
            match self.tok.next() {
                Token::Identifier(local) => main_locals.push(local),
                other => panic!("(Expected local variable name but got {:?}", other),
            }

            if self.tok.peek().get_type() == TokenType::Comma {
                self.tok.next();
            }
        }
        self.tok.next();

        let mut main_body = Vec::<Statement>::new();
        while self.tok.peek().get_type() != TokenType::Eof {
            main_body.push(self.parse_statement());
        }
        
        Program { classes, main_locals, main_body }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::Tokenizer;
    
    #[test]
    fn assignment() {
        let tok = Tokenizer::new("x = 420".to_string());
        let mut parser = Parser::new(tok);
        match parser.parse_statement() {
            Statement::Assignment { variable, expression } => {
                assert_eq!(variable, "x");
                assert!(matches!(expression, Expression::Constant(420)));
            }
            _ => panic!("Expected Assignment"),
        }
    }
    
    #[test]
    fn discard() {
        let tok = Tokenizer::new("_ = 69".to_string());
        let mut parser = Parser::new(tok);
        match parser.parse_statement() {
            Statement::Discard(expr) => {
                assert!(matches!(expr, Expression::Constant(69)));
            }
            _ => panic!("Expected Discard"),
        }
    }
    
    #[test]
    fn return_statement() {
        let tok = Tokenizer::new("return (17 +83)".to_string());
        let mut parser = Parser::new(tok);
        match parser.parse_statement() {
            Statement::Return(expr) => {
                assert!(matches!(expr, Expression::Binop { .. }));
            }
            _ => panic!("Expected Return"),
        }
    }
    
    #[test]
    fn print_statement() {
        let tok = Tokenizer::new("print(x)".to_string());
        let mut parser = Parser::new(tok);
        match parser.parse_statement() {
            Statement::Print(expr) => {
                match expr {
                    Expression::Variable(name) => assert_eq!(name, "x"),
                    _ => panic!("Expected Variable"),
                }
            }
            _ => panic!("Expected Print"),
        }
    }
    
    #[test]
    fn field_write() {
        let tok = Tokenizer::new("!e.f = 100".to_string());
        let mut parser = Parser::new(tok);
        match parser.parse_statement() {
            Statement::FieldWrite { base, field, value } => {
                match base {
                    Expression::Variable(name) => assert_eq!(name, "e"),
                    _ => panic!("Expected Variable for base"),
                }
                assert_eq!(field, "f");
                assert!(matches!(value, Expression::Constant(100)));
            }
            _ => panic!("Expected FieldWrite"),
        }
    }
    
    #[test]
    fn ifonly_statement() {
        let tok = Tokenizer::new("ifonly x: {return   25 }".to_string());
        let mut parser = Parser::new(tok);
        match parser.parse_statement() {
            Statement::IfOnly { condition, body } => {
                match condition {
                    Expression::Variable(name) => assert_eq!(name, "x"),
                    _ => panic!("Expected Variable for condition"),
                }
                assert_eq!(body.len(), 1);
                assert!(matches!(body[0], Statement::Return(_)));
            }
            _ => panic!("Expected IfOnly"),
        }
    }
    
    #[test]
    fn if_else_statement() {
        let tok = Tokenizer::new("if x: { return 62 }    \nelse { return 38 }".to_string());
        let mut parser = Parser::new(tok);
        match parser.parse_statement() {
            Statement::If { condition, then_body, else_body } => {
                match condition {
                    Expression::Variable(name) => assert_eq!(name, "x"),
                    _ => panic!("Expected Variable for condition"),
                }
                assert_eq!(then_body.len(), 1);
                assert_eq!(else_body.len(), 1);
                assert!(matches!(then_body[0], Statement::Return(_)));
                assert!(matches!(else_body[0], Statement::Return(_)));
            }
            _ => panic!("Expected If"),
        }
    }
    
    #[test]
    fn while_statement() {
        let tok = Tokenizer::new("while x: { x = ( x - 1 ) }".to_string());
        let mut parser = Parser::new(tok);
        match parser.parse_statement() {
            Statement::While { condition, body } => {
                match condition {
                    Expression::Variable(name) => assert_eq!(name, "x"),
                    _ => panic!("Expected Variable for condition"),
                }
                assert_eq!(body.len(), 1);
                assert!(matches!(body[0], Statement::Assignment { .. }));
            }
            _ => panic!("Expected While"),
        }
    }
}