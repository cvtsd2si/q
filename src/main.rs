use q::asm::{emitter::emit_asm_prog, fixup::fixup, r#gen::codegen_ir_prog};
use q::ir::r#gen::irfy_ast;
use q::lexer::tokenizer::Tokenizer;
use q::parser::recursive_descent::Parser;
use std::collections::VecDeque;
use std::path::Path;

fn main() {
    match std::env::args().nth(1) {
        Some(path) => {
            if let Err(e) = run(path) {
                eprintln!("err: {}", e);
            }
        }
        None => eprintln!("usage: q <path>"),
    }
}

fn run(path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    let src = std::fs::read_to_string(path)?;

    let tokens = Tokenizer::new(&src).collect::<Result<VecDeque<_>, _>>()?;
    let ast = Parser::new(&src, tokens).parse()?;
    let ir_prog = irfy_ast(&ast);
    let mut asm_prog = codegen_ir_prog(&ir_prog);

    emit_asm_prog(fixup(&mut asm_prog));

    Ok(())
}
