use q::bytecode::r#gen::{Bytecode, BytecodeProg};
use q::ir::r#gen::{Codegen, irfy};
use q::lexer::tokenizer::Tokenizer;
use q::parser::recursive_descent::Parser;
use q::runtime::value::ValueTag;
use q::runtime::vm::Vm;
use std::collections::VecDeque;
use std::path::PathBuf;
use structopt::StructOpt;

fn main() {
    let opts = Opt::from_args();
    match run(&opts) {
        Ok(r) => std::process::exit(r as i32),
        Err(e) => eprintln!("err: {}", e),
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

    let bytecode: BytecodeProg = ir.codegen();

    if opts.bytecode {
        println!("{:#?}", bytecode);
        return Ok(0);
    }

    let mut vm = Vm::new(bytecode, ir);
    let r = vm.exec();

    Ok(r.payload as i32)
}

#[derive(Debug, Clone, StructOpt)]
struct Opt {
    path: PathBuf,

    #[structopt(name = "tokenize", long)]
    tokenize: bool,

    #[structopt(name = "parse", long)]
    parse: bool,

    #[structopt(name = "ir", long)]
    ir: bool,

    #[structopt(name = "bytecode", long)]
    bytecode: bool,
}
