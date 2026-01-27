mod token;
mod tokenizer;
mod expression;
mod parser;
mod statement;
mod ast;
mod ir;
mod ir_builder;
mod cfg;

use tokenizer::Tokenizer;
use parser::Parser;
use ir_builder::IRBuilder;
use cfg::CFG;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: ./comp [-noopt] <source_file>");
        std::process::exit(1);
    }

    let (no_opt, filename) = if args[1] == "-noopt" {
        if args.len() < 3 {
            eprintln!("Usage: ./comp -noopt <source_file>");
            std::process::exit(1);
        }
        (true, &args[2])
    } else {
        (false, &args[1])
    };

    let source = std::fs::read_to_string(filename)
        .unwrap_or_else(|_| {
            eprintln!("Error: Could not read file '{}'", filename);
            std::process::exit(1);
        });

    let tokenizer = Tokenizer::new(source);
    let mut parser = Parser::new(tokenizer);
    let ast = parser.parse_program();

    let mut ir_builder = IRBuilder::new();
    let mut ir_program = ir_builder.gen_program(&ast);

    for function in &mut ir_program.functions {
        let mut cfg = CFG::new(function);
        cfg.convert_to_ssa(function);
        
        if !no_opt {
            cfg.fold_constants(function);
        }
    }

    ir_program.print();
}