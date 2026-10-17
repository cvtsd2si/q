use libc::{
    MAP_ANON, MAP_FAILED, MAP_PRIVATE, PROT_EXEC, PROT_READ, PROT_WRITE, mmap, mprotect, munmap,
};
use structopt::StructOpt;
use q::asm::{emitter::Emit, fixup::ReplacePseudo, r#gen::Codegen};
use q::ir::r#gen::irfy;
use q::lexer::tokenizer::Tokenizer;
use q::parser::recursive_descent::Parser;
use rsasm::assembler::Options;
use std::collections::VecDeque;
use std::path::PathBuf;

fn main() {
    let opts = Opt::from_args();
    match run(&opts) {
        Ok(r) => std::process::exit(r),
        Err(e) => eprintln!("err: {}", e)
    }
}

fn run(opts: &Opt) -> Result<i32, Box<dyn std::error::Error>> {
    let src = std::fs::read_to_string(&opts.path)?;

    let tokens = Tokenizer::new(&src).collect::<Result<VecDeque<_>, _>>()?;

    if opts.tokenize {
        println!("{:#?}", tokens);
        return Ok(0);
    }

    let ast = Parser::new(&src, tokens).parse()?;

    if opts.parse {
        println!("{:#?}", ast);
        return Ok(0);
    }

    let ir = irfy(&ast);

    if opts.ir {
        println!("{:#?}", ir);
        return Ok(0);
    }

    let mut asm = ir.codegen();

    if opts.codegen {
        println!("{:#?}", asm);
        return Ok(0);
    }

    let mut asm_txt = String::new();

    asm.replace_pseudo().emit(&mut asm_txt)?;

    if opts.emit {
        println!("{asm_txt}");
        return Ok(0);
    }

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

#[derive(Debug, Clone, StructOpt)]
struct Opt {
    #[structopt(short = "o", long = "output", parse(from_os_str))] 
    output: Option<PathBuf>,

    path: PathBuf,

    #[structopt(name = "tokenize", long)]
    tokenize: bool,
    
    #[structopt(name = "parse", long)]
    parse: bool,

    #[structopt(name = "ir", long)]
    ir: bool,

    #[structopt(name = "codegen", long)]
    codegen: bool,

    #[structopt(name = "emit", long)]
    emit: bool,
}
