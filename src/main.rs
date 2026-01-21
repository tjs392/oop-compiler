mod token;
mod tokenizer;
mod expression;
mod parser;
mod statement;
mod ast;
mod ir;
mod codegen;

use token::{TokenType};
use tokenizer::Tokenizer;
use parser::Parser;
use codegen::CodeGenerator;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: <comp> {{tokenize|parseExpr}} [args...]");
        std::process::exit(1);
    }

    let input = args[2..].join(" ");

    let mut tok = Tokenizer::new(input);

    match args[1].as_str() {
        "compile" => {
            if args.len() < 3 {
                eprintln!("Usage: <comp> compile <source_code>");
                std::process::exit(1);
            }
            let input = args[2..].join(" ");
            compile(input);
        }

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

fn compile(input: String) {
    let source = if std::path::Path::new(&input).exists() {
        std::fs::read_to_string(&input).expect(&format!("file {} does not exist", input))
    } else {
        input
    };

    let tokenizer = Tokenizer::new(source);
    let mut parser = Parser::new(tokenizer);
    let ast = parser.parse_program();

    let mut code_generator = CodeGenerator::new();
    let ir_program = code_generator.gen_program(&ast);

    ir_program.print();
}