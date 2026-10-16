use q::asm::{emitter::Emit, fixup::replace_pseudo, r#gen::codegen};
use q::ir::r#gen::irfy;
use q::lexer::tokenizer::Tokenizer;
use q::parser::recursive_descent::Parser;
use std::collections::VecDeque;
use std::path::Path;

fn main() {
    match std::env::args().nth(1) {
        Some(path) => {
            let stage = if let Some(s) = std::env::args().nth(2) {
                match s.as_str() {
                    "--tokenize" => Some(Stage::Tokenize),
                    "--parse" => Some(Stage::Parse),
                    "--ir" => Some(Stage::Ir),
                    "--codegen" => Some(Stage::Codegen),
                    "--emit" => Some(Stage::Emit),
                    _ => {
                        eprintln!("err: unrecognized stage: {}", s);
                        std::process::exit(1);
                    }
                }
            } else {
                None
            };

            if let Err(e) = run(path, stage) {
                eprintln!("err: {}", e);
            }
        }
        None => eprintln!("usage: q <path>"),
    }
}

fn run(path: impl AsRef<Path>, stage: Option<Stage>) -> Result<(), Box<dyn std::error::Error>> {
    let src = std::fs::read_to_string(path)?;

    let tokens = Tokenizer::new(&src).collect::<Result<VecDeque<_>, _>>()?;

    if stage == Some(Stage::Tokenize) {
        println!("{:?}", tokens);
        return Ok(());
    }

    let ast = Parser::new(&src, tokens).parse()?;

    if stage == Some(Stage::Parse) {
        println!("{:#?}", ast);
        return Ok(());
    }

    let ir = irfy(&ast);

    if stage == Some(Stage::Ir) {
        println!("{:#?}", ir);
        return Ok(());
    }

    let mut asm = codegen(&ir);

    if stage == Some(Stage::Codegen) {
        println!("{:#?}", asm);
        return Ok(());
    }

    replace_pseudo(&mut asm).emit();

    if stage == Some(Stage::Emit) {
        todo!()
    }

    Ok(())
}

#[derive(PartialEq)]
enum Stage {
    Tokenize,
    Parse,
    Ir,
    Codegen,
    Emit,
}
