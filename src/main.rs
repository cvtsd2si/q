use q::{
    PrintSrc, QError,
    codegen::{
        Codegen, CodegenContext,
        bytecode::r#gen::BytecodeProg,
        ir::{
            r#gen::Irfy,
            optimizer::{Optimization, Optimize},
        },
    },
    parser::{ast::TypeInterner, recursive_descent::Parser},
    runtime::vm::Vm,
    semantics::{
        label_collector::{LabelCollect, LabelCollector},
        loop_labeler::{LoopLabel, LoopLabeler},
        resolver::{Resolve, Resolver},
        typechecker::{Typecheck, Typechecker},
    },
    tokenizer::Tokenizer,
};

use std::{collections::VecDeque, path::PathBuf};

use structopt::StructOpt;

fn main() -> Result<(), std::io::Error> {
    let opts = Opt::from_args();
    let src = std::fs::read_to_string(&opts.path)?;

    match run(&src, &opts) {
        Ok(r) => std::process::exit(r),
        Err(ref e) => {
            if e.carries_span() {
                e.print_src(&src);
            }

            eprintln!("{}", e);

            std::process::exit(e.code as i32);
        }
    }
}

fn run(src: &str, opts: &Opt) -> Result<i32, QError> {
    let tokens = Tokenizer::new(src).collect::<Result<VecDeque<_>, _>>()?;

    if opts.tokenize {
        println!("{:#?}", tokens);
        return Ok(0);
    }

    let mut types = TypeInterner::new();

    let ast = Parser::new(src, tokens, &mut types).parse()?;

    if opts.parse {
        println!("{:#?}", ast);
        return Ok(0);
    }

    let mut label_collector = LabelCollector::new();
    let mut loop_labeler = LoopLabeler::new();

    let labeled_ast = ast
        .label_collect(&mut label_collector)?
        .loop_label(&mut loop_labeler)?;

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

    let mut typechecker = Typechecker::new(&mut types);
    let typechecked_ast = resolved_ast.typecheck(&mut typechecker)?;

    if opts.typecheck {
        println!("{:#?}", typechecked_ast);
        return Ok(0);
    }

    let ir = typechecked_ast
        .irfy(&mut types)
        .optimize(&opts.optimizations);

    if opts.ir {
        println!("{:#?}", ir);
        return Ok(0);
    }

    let bytecode: BytecodeProg = ir.codegen(&mut CodegenContext::new(&types))?;

    if opts.bytecode {
        println!("{:#?}", bytecode);
        return Ok(0);
    }

    let mut vm = Vm::new(bytecode, ir, opts.jit, &types)?;
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

    #[structopt(name = "optimizations", long)]
    optimizations: Vec<Optimization>,

    #[structopt(name = "bytecode", long)]
    bytecode: bool,

    #[structopt(name = "jit", long)]
    jit: bool,
}
