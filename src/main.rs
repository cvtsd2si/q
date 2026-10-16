use q::asm::{emitter::Emit, fixup::ReplacePseudo, r#gen::codegen};
use q::ir::r#gen::irfy;
use q::lexer::tokenizer::Tokenizer;
use q::parser::recursive_descent::Parser;
use std::collections::VecDeque;
use std::fs::File;
use std::path::{Path, PathBuf};

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
                    "--asm" => Some(Stage::Asm),
                    "--ln" => Some(Stage::Ln),
                    _ => {
                        eprintln!("err: unrecognized stage: {}", s);
                        std::process::exit(1);
                    }
                }
            } else {
                None
            };

            if let Err(e) = run(path.into(), stage) {
                eprintln!("err: {}", e);
            }
        }
        None => eprintln!("usage: q <path>"),
    }
}

fn run(path: PathBuf, stage: Option<Stage>) -> Result<(), Box<dyn std::error::Error>> {
    let src = std::fs::read_to_string(&path)?;

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

    let mut f = File::create(path.with_extension("s"))?;

    asm.replace_pseudo().emit(&mut f)?;

    if stage == Some(Stage::Emit) {
        return Ok(());
    }

    std::process::Command::new("gcc")
        .arg("-c")
        .arg(path.with_extension("s"))
        .arg("-o")
        .arg(path.with_extension("o"))
        .status()?;

    if stage == Some(Stage::Asm) {
        return Ok(());
    }

    std::process::Command::new("gcc")
        .arg("-o")
        .arg(path.with_extension(""))
        .arg(path.with_extension("s"))
        .status()?;

    if stage == Some(Stage::Ln) {
        return Ok(());
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
    Asm,
    Ln,
}
