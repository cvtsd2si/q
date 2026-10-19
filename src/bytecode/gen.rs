use std::{collections::HashMap, fmt::Debug};

use crate::{
    ir::r#gen::{Codegen, IRFunc, IRInstr, IRProg, IRVal},
    parser::ast::{BinaryExpressionKind, Literal},
};

#[derive(Debug, Clone, Copy)]
pub enum Op {
    Add,
    Sub,
    Const(Literal),
    Ret,
    Print,
    Deepget(usize),
    Deepset(usize),
    Jz(usize),
    Jmp(usize),
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

type Locals = HashMap<String, usize>;

impl Codegen<BytecodeFunc> for IRFunc {
    fn codegen(&self) -> BytecodeFunc {
        let mut f = BytecodeFunc {
            name: self.name.to_owned(),
            body: vec![],
            ir_map: vec![],
        };

        let mut locals = Locals::new();

        let mut labels = HashMap::new();
        let mut fixups = vec![];

        for (ir_idx, instr) in self.body.iter().enumerate() {
            let ops: Bytecode = instr.codegen_with_locals(&mut locals);

            match instr {
                IRInstr::Label(target) => {
                    labels.insert(target, f.body.len());
                }

                IRInstr::Jz { target, .. } | IRInstr::Jmp { target } => {
                    let pc = f.body.len() + ops.len() - 1;
                    fixups.push((pc, target.clone()));
                }

                _ => {}
            }

            f.ir_map.extend(std::iter::repeat(ir_idx).take(ops.len()));

            f.body.extend(ops);
        }

        for (pc, name) in &fixups {
            let dst = labels.get(name).unwrap();

            match f.body[*pc] {
                Op::Jz(ref mut target) | Op::Jmp(ref mut target) => {
                    *target = *dst;
                }

                _ => unreachable!(),
            }
        }

        f
    }
}

impl IRInstr {
    fn codegen_with_locals(&self, locals: &mut Locals) -> Bytecode {
        match self {
            IRInstr::Cpy { src, dst } => {
                let op = src.load_op(locals);

                self.register_local(locals, dst);

                vec![op]
            }

            IRInstr::Bin {
                kind,
                lhs,
                rhs,
                dst,
            } => {
                let op = match kind {
                    BinaryExpressionKind::Add => Op::Add,
                    BinaryExpressionKind::Sub => Op::Sub,
                    _ => unreachable!(),
                };

                let lhs = lhs.load_op(locals);
                let rhs = rhs.load_op(locals);

                self.register_local(locals, dst);

                vec![lhs, rhs, op]
            }

            IRInstr::Ret { val } => vec![val.load_op(locals), Op::Ret],

            IRInstr::Print { val } => vec![val.load_op(locals), Op::Print],

            IRInstr::Jz { cond, target } => vec![cond.load_op(locals), Op::Jz(usize::MAX)],

            IRInstr::Jmp { target } => vec![Op::Jmp(usize::MAX)],

            IRInstr::Label(target) => vec![],
        }
    }

    fn register_local(&self, locals: &mut Locals, local: &IRVal) {
        match local {
            IRVal::Const(konst) => unreachable!(),
            IRVal::Var(var) => {
                let idx = locals.len();
                locals.insert(var.to_owned(), idx);
            }
        }
    }
}

impl IRVal {
    fn load_op(&self, locals: &Locals) -> Op {
        match self {
            IRVal::Const(konst) => Op::Const(*konst),
            IRVal::Var(var) => {
                if let Some(idx) = locals.get(var) {
                    Op::Deepget(*idx)
                } else {
                    unreachable!();
                }
            }
        }
    }

    fn store_op(&self, locals: &Locals) -> Op {
        match self {
            IRVal::Const(konst) => unreachable!(),
            IRVal::Var(var) => {
                if let Some(idx) = locals.get(var) {
                    Op::Deepset(*idx)
                } else {
                    unreachable!();
                }
            }
        }
    }
}
