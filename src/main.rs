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

    let mut use_ssa = true;
    let mut use_vn = true;
    let mut use_fold = true;
    let mut filename: Option<&String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--ssa"     => use_ssa = true,
            "--no-ssa"  => use_ssa = false,
            "--vn"      => use_vn = true,
            "--no-vn"   => use_vn = false,
            "--fold"    => use_fold = true,
            "--no-fold" => use_fold = false,
            arg if arg.starts_with("--") => {
                eprintln!("Unknown flag: {}", arg);
                eprintln!("Usage: ./comp [--ssa|--no-ssa] [--vn|--no-vn] [--fold|--no-fold] <source_file>");
                std::process::exit(1);
            }
            _ => {
                if filename.is_some() {
                    eprintln!("Error: multiple filenames provided");
                    std::process::exit(1);
                }
                filename = Some(&args[i]);
            }
        }
        i += 1;
    }

    let filename = filename.unwrap_or_else(|| {
        eprintln!("Usage: ./comp [--ssa|--no-ssa] [--vn|--no-vn] [--fold|--no-fold] <source_file>");
        std::process::exit(1);
    });

    let source = std::fs::read_to_string(filename)
        .unwrap_or_else(|_| {
            eprintln!("Error: could not read file '{}'", filename);
            std::process::exit(1);
        });

    let tokenizer = Tokenizer::new(source);
    let mut parser = Parser::new(tokenizer);
    let ast = parser.parse_program();

    let mut ir_builder = IRBuilder::new();
    let mut ir_program = ir_builder.gen_program(&ast);

    for function in &mut ir_program.functions {
        let mut cfg = CFG::new(function);

        if use_ssa {
            cfg.convert_to_ssa(function);
        }

        if use_vn {
            cfg.value_numbering(function);
        }

        if use_fold {
            cfg.fold_constants(function);
        }
    }

    ir_program.print();
}