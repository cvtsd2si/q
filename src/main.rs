use libc::{
    MAP_ANON, MAP_FAILED, MAP_PRIVATE, PROT_EXEC, PROT_READ, PROT_WRITE, mmap, mprotect, munmap,
};
use q::asm::{emitter::Emit, fixup::ReplacePseudo, r#gen::Codegen};
use q::ir::r#gen::irfy;
use q::lexer::tokenizer::Tokenizer;
use q::parser::recursive_descent::Parser;
use rsasm::assembler::Options;
use std::collections::VecDeque;
use std::path::PathBuf;

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

            match run(path.into(), stage) {
                Ok(r) => std::process::exit(r),
                Err(e) => eprintln!("err: {}", e),
            }
        }
        None => eprintln!("usage: q <path>"),
    }
}

fn run(path: PathBuf, stage: Option<Stage>) -> Result<i32, Box<dyn std::error::Error>> {
    let src = std::fs::read_to_string(&path)?;

    let tokens = Tokenizer::new(&src).collect::<Result<VecDeque<_>, _>>()?;

    if stage == Some(Stage::Tokenize) {
        println!("{:?}", tokens);
        return Ok(0);
    }

    let ast = Parser::new(&src, tokens).parse()?;

    if stage == Some(Stage::Parse) {
        println!("{:#?}", ast);
        return Ok(0);
    }

    let ir = irfy(&ast);

    if stage == Some(Stage::Ir) {
        println!("{:#?}", ir);
        return Ok(0);
    }

    let mut asm = ir.codegen();

    if stage == Some(Stage::Codegen) {
        println!("{:#?}", asm);
        return Ok(0);
    }

    let mut asm_txt = String::new();

    asm.replace_pseudo().emit(&mut asm_txt)?;

    let mut assembler =
        rsasm::assembler::Assembler::new(rsasm::arch::lookup("x86-64").unwrap(), Options::new());

    assembler.assemble_str("", &asm_txt);

    assert!(assembler.finish());

    let raw_bytes = assembler.section_bytes(rsasm::section::SectionId(0));

    unsafe {
        let len = raw_bytes.len();

        let ptr = mmap(
            std::ptr::null_mut(),
            len,
            PROT_READ | PROT_WRITE,
            MAP_PRIVATE | MAP_ANON,
            -1,
            0,
        );

        if ptr == MAP_FAILED {
            panic!("mmap failed");
        }

        std::ptr::copy_nonoverlapping(raw_bytes.as_ptr(), ptr.cast::<u8>(), len);

        if mprotect(ptr, len, PROT_READ | PROT_EXEC) != 0 {
            panic!("mprotect failed");
        }

        type Main = unsafe extern "C" fn() -> i32;

        let main: Main = std::mem::transmute(ptr);

        let r: i32 = main();

        munmap(ptr, len);

        return Ok(r);
    }
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
