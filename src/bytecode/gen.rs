use std::fmt::Debug;

use crate::{
    ir::r#gen::{Codegen, IRFunc, IRInstr, IRProg, IRVal},
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

impl Codegen<BytecodeProg> for IRProg {
    fn codegen(&self) -> BytecodeProg {
        let mut bytecode_prog = BytecodeProg { funcs: vec![] };

        for f in &self.funcs {
            bytecode_prog.funcs.push(f.codegen());
        }

        bytecode_prog
    }
}

impl Codegen<BytecodeFunc> for IRFunc {
    fn codegen(&self) -> BytecodeFunc {
        let mut f = BytecodeFunc {
            name: self.name.to_owned(),
            body: vec![],
            ir_map: vec![],
        };

        for (ir_idx, instr) in self.body.iter().enumerate() {
            let ops: Bytecode = instr.codegen();

            for _ in 0..ops.len() {
                f.ir_map.push(ir_idx);
            }

            f.body.extend(ops);
        }

        f
    }
}

impl Codegen<Bytecode> for IRInstr {
    fn codegen(&self) -> Bytecode {
        let mut bytecode = vec![];

        match self {
            IRInstr::Bin {
                kind,
                lhs,
                rhs,
                dst: _,
            } => match kind {
                BinaryExpressionKind::Add | BinaryExpressionKind::Sub => {
                    bytecode.push(lhs.codegen());
                    bytecode.push(rhs.codegen());
                    bytecode.push(match kind {
                        BinaryExpressionKind::Add => Op::Add,
                        BinaryExpressionKind::Sub => Op::Sub,
                        _ => unimplemented!(),
                    });
                }

                _ => todo!(),
            },

            IRInstr::Ret { val } => {
                bytecode.push(val.codegen());
                bytecode.push(Op::Ret);
            }

            IRInstr::Print { val } => {
                bytecode.push(val.codegen());
                bytecode.push(Op::Print);
            }

            _ => todo!(),
        }

        bytecode
    }
}

impl Codegen<Op> for IRVal {
    fn codegen(&self) -> Op {
        match self {
            IRVal::Const(konst) => Op::Const(*konst),
            IRVal::Var(var) => Op::Deepget(0),
        }
    }
}
