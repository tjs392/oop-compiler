mod token;
mod tokenizer;
mod expression;
mod parser;
mod statement;
mod ast;
mod ir;

use token::{TokenType};
use tokenizer::Tokenizer;
use parser::Parser;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: <comp> {{tokenize|parseExpr}} [args...]");
        std::process::exit(1);
    }

    let input = args[2..].join(" ");

    let mut tok = Tokenizer::new(input);

    match args[1].as_str() {
        "tokenize" => {
            while tok.peek().get_type() != TokenType::Eof {
                println!("{:?}", tok.next());
            }
        }

        "parseExpr" => {
            let mut parser = Parser::new(tok);
            println!("{:?}", parser.parse_expr());
        }

        "parseStmt" => {
            let mut parser = Parser::new(tok);
            println!("{:?}", parser.parse_statement());
        }

        "parseClass" => {
            let mut parser = Parser::new(tok);
            println!("{:#?}", parser.parse_class());
        }

        "parseProgram" => {
            let mut parser = Parser::new(tok);
            println!("{:#?}", parser.parse_program());
        }

        _ => {
            eprintln!("Unsupported subcommand: {}", args[1]);
        }
    }
}