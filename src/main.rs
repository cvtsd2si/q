use q::bytecode::r#gen::BytecodeProg;
use q::ir::cfg::Cfg;
use q::ir::r#gen::{Codegen, CodegenContext, irfy};
use q::lexer::tokenizer::Tokenizer;
use q::parser::recursive_descent::Parser;
use q::runtime::vm::Vm;
use q::semantics::label_collector::{LabelCollect, LabelCollector};
use q::semantics::loop_labeler::{LoopLabel, LoopLabeler};
use q::semantics::resolver::{Resolve, Resolver};
use q::semantics::typechecker::{Typecheck, Typechecker};
use std::collections::VecDeque;
use std::path::PathBuf;
use structopt::StructOpt;

fn main() {
    let opts = Opt::from_args();
    match run(&opts) {
        Ok(r) => std::process::exit(r),
        Err(e) => {
            eprintln!("err: {}", e);
            std::process::exit(1);
        }
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

    let mut label_collector = LabelCollector::new();
    let ast = ast.label_collect(&mut label_collector)?;

    let mut loop_labeler = LoopLabeler::new();
    let labeled_ast = ast.loop_label(&mut loop_labeler)?;

    if opts.loop_label {
        println!("{:#?}", labeled_ast);
        return Ok(0);
    }

    let mut resolver = Resolver::new();
    let resolved_ast = labeled_ast.resolve(&mut resolver)?;

    if opts.resolve {
        println!("{:#?}", resolved_ast);
        return Ok(0);
    }

    let mut typechecker = Typechecker::new();
    let typechecked_ast = resolved_ast.typecheck(&mut typechecker)?;

    if opts.typecheck {
        println!("{:#?}", typechecked_ast);
        return Ok(0);
    }

    let ir = irfy(&typechecked_ast);

    if opts.ir_graph {
        for func in &ir.funcs {
            let cfg = Cfg::build(func)?;
            let path = format!("{}.dot", func.name);

            std::fs::write(path, cfg.to_dot(func))?;
        }
    }

    if opts.ir {
        println!("{:#?}", ir);
        return Ok(0);
    }

    let bytecode: BytecodeProg = ir.codegen(&mut CodegenContext::new())?;

    if opts.bytecode {
        println!("{:#?}", bytecode);
        return Ok(0);
    }

    let mut vm = Vm::new(bytecode, ir, opts.jit)?;
    let r = vm.exec()?;

    Ok(r.payload as i32)
}

#[derive(Debug, Clone, StructOpt)]
struct Opt {
    path: PathBuf,

    #[structopt(name = "tokenize", long)]
    tokenize: bool,

    #[structopt(name = "parse", long)]
    parse: bool,

    #[structopt(name = "loop-label", long)]
    loop_label: bool,

    #[structopt(name = "resolve", long)]
    resolve: bool,

    #[structopt(name = "typecheck", long)]
    typecheck: bool,

    #[structopt(name = "ir", long)]
    ir: bool,

    #[structopt(name = "ir-graph", long)]
    ir_graph: bool,

    #[structopt(name = "bytecode", long)]
    bytecode: bool,

    #[structopt(name = "jit", long)]
    jit: bool,
}
