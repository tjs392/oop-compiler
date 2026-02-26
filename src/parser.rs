use crate::token::{Token, TokenType};
use crate::tokenizer::Tokenizer;
use crate::expression::Expression;
use crate::statement::Statement;
use crate::ast::{Class, Method, Program, Type};

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

            Token::Null => {
                match self.tok.next() {
                    Token::Colon => {},
                    other => panic!("Expected a ':' after null, but got {:?}", other),
                }
                let class_name = match self.tok.next() {
                    Token::Identifier(n) => n,
                    other => panic!("Expected class name after null:, got {:?}", other),
                };
                Expression::Null(class_name)
            }

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

        let mut args = Vec::<(String, Type)>::new();
        while self.tok.peek().get_type() != TokenType::RightParen {
            match self.tok.next() {
                Token::Identifier(arg) => {
                    match self.tok.next() {
                        Token::Colon => {},
                        other => panic!("Expected : after arg name, got {:?}", other),
                    }
                    let typ = self.parse_type();
                    args.push((arg, typ));
                },
                other => panic!("Expected argument, got {:?}", other)
            }

            if self.tok.peek().get_type() == TokenType::Comma {
                self.tok.next();
            }
        }
        self.tok.next();

        let return_type = if self.tok.peek().get_type() == TokenType::Returning {
            self.tok.next();
            self.parse_type()
        } else {
            Type::Int
        };

        match self.tok.next() {
            Token::With => {},
            other => panic!("Expected 'with' after arguments, got {:?}", other),
        }

        match self.tok.next() {
            Token::Locals => {},
            other => panic!("Expected 'locals' after 'with', got {:?}", other),
        }

        let mut locals = Vec::<(String, Type)>::new();
        while self.tok.peek().get_type() != TokenType::Colon {
            match self.tok.next() {
                Token::Identifier(local) => {
                    match self.tok.next() {
                        Token::Colon => {},
                        other => panic!("Expected : after local name, got {:?}", other),
                    }
                    let typ = self.parse_type();
                    locals.push((local, typ));
                },
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

        Method { name, args, locals, body, return_type }
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

        let mut fields = Vec::<(String, Type)>::new();
        while self.tok.peek().get_type() != TokenType::Method && self.tok.peek().get_type() != TokenType::RightBracket {
            match self.tok.next() {
                Token::Identifier(field) => {
                    match self.tok.next() {
                        Token::Colon => {},
                        other => panic!("Expected : after field name, got {:?}", other),
                    }
                    let typ = self.parse_type();
                    fields.push((field, typ));
                },
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

        let mut main_locals = Vec::<(String,Type)>::new();
        while self.tok.peek().get_type() != TokenType::Colon {
            match self.tok.next() {
                Token::Identifier(local) => {
                    match self.tok.next() {
                        Token::Colon => {},
                        other => panic!("Expected : after local name, got {:?}", other),
                    }
                    let typ = self.parse_type();
                    main_locals.push((local, typ));
                },
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

    pub fn parse_type(&mut self) -> Type {
        match self.tok.next() {
            Token::Identifier(name) => {
                if name == "int" {
                    Type::Int
                } else {
                    Type::ClassType(name)
                }
            }
            other => panic!("Expected type, not {:?}", other)
        }
    }
}