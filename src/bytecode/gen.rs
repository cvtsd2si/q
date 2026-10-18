use std::fmt::Debug;

use crate::{
    ir::r#gen::{IRFunc, IRInstr, IRProg, IRVal},
    parser::ast::BinaryExpressionKind,
};

#[derive(Debug, Clone, Copy)]
pub enum Op {
    Add,
    Sub,
    Const(i64),
    Ret,
    Print,
    Deepget(usize),
}

pub type Bytecode = Vec<Op>;

#[derive(Debug)]
pub struct BytecodeProg {
    pub funcs: Vec<BytecodeFunc>,
}

#[derive(Debug)]
pub struct BytecodeFunc {
    pub name: String,
    pub body: Bytecode,

    pub ir_map: Vec<usize>,
}

pub trait Codegen {
    type Output;

    fn codegen<T>(&self) -> Self::Output;
}

impl Codegen for IRProg {
    type Output = BytecodeProg;

    fn codegen<T>(&self) -> Self::Output {
        let mut bytecode_prog = BytecodeProg { funcs: vec![] };

        for f in &self.funcs {
            bytecode_prog.funcs.push(f.codegen::<Bytecode>());
        }

        bytecode_prog
    }
}

impl Codegen for IRFunc {
    type Output = BytecodeFunc;

    fn codegen<T>(&self) -> Self::Output {
        let mut f = BytecodeFunc {
            name: self.name.to_owned(),
            body: vec![],
            ir_map: vec![],
        };

        for (ir_idx, instr) in self.body.iter().enumerate() {
            let ops = instr.codegen::<Bytecode>();

            for _ in 0..ops.len() {
                f.ir_map.push(ir_idx);
            }

            f.body.extend(ops);
        }

        f
    }
}

impl Codegen for IRInstr {
    type Output = Bytecode;

    fn codegen<T>(&self) -> Self::Output {
        let mut bytecode = vec![];

        match self {
            IRInstr::Bin {
                kind,
                lhs,
                rhs,
                dst: _,
            } => match kind {
                BinaryExpressionKind::Add => {
                    bytecode.push(lhs.codegen::<Bytecode>());
                    bytecode.push(rhs.codegen::<Bytecode>());
                    bytecode.push(Op::Add);
                }

                BinaryExpressionKind::Sub => {}

                _ => todo!(),
            },

            IRInstr::Ret { val } => {
                bytecode.push(val.codegen::<Bytecode>());
                bytecode.push(Op::Ret);
            }

            IRInstr::Print { val } => {
                bytecode.push(val.codegen::<Bytecode>());
                bytecode.push(Op::Print);
            }

            _ => todo!(),
        }

        bytecode
    }
}

impl Codegen for IRVal {
    type Output = Op;

    fn codegen<T>(&self) -> Self::Output {
        match self {
            IRVal::Const(konst) => Op::Const(*konst),
            IRVal::Var(var) => Op::Deepget(0),
        }
    }
}
