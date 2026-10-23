use std::{collections::HashMap, fmt::Debug};

use crate::{
    ir::r#gen::{Codegen, CodegenContext, Ctx, IRFunc, IRInstr, IRProg, IRVal},
    parser::ast::{BinaryExpressionKind, Literal},
};

#[derive(Debug, Clone, Copy)]
pub enum Op {
    Add,
    Sub,
    Const(Literal),
    Ret,
    RetVoid,
    Print,
    Deepget(usize),
    Deepset(usize),
    Jz(usize),
    Jmp(usize),
    Call { target: usize, argc: usize },
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

impl Codegen<BytecodeProg, CodegenContext> for IRProg {
    fn codegen(
        &self,
        ctx: &mut CodegenContext,
    ) -> Result<BytecodeProg, Box<dyn std::error::Error>> {
        let mut bytecode_prog = BytecodeProg { funcs: vec![] };

        for f in &self.funcs {
            ctx.register::<BytecodeFunc>(&f.name);
        }

        for f in &self.funcs {
            bytecode_prog.funcs.push(f.codegen(ctx)?);
        }

        Ok(bytecode_prog)
    }
}

impl Codegen<BytecodeFunc, CodegenContext> for IRFunc {
    fn codegen(
        &self,
        ctx: &mut CodegenContext,
    ) -> Result<BytecodeFunc, Box<dyn std::error::Error>> {
        ctx.locals.clear();

        for param in &self.params {
            ctx.register::<String>(&param.name);
        }

        let mut f = BytecodeFunc {
            name: self.name.to_owned(),
            body: vec![],
            ir_map: vec![],
        };

        let mut labels = HashMap::new();
        let mut fixups = vec![];

        for (ir_idx, instr) in self.body.iter().enumerate() {
            let ops: Bytecode = instr.codegen(ctx)?;

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

            f.ir_map.extend(std::iter::repeat_n(ir_idx, ops.len()));

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

        Ok(f)
    }
}

impl Codegen<Bytecode, CodegenContext> for IRInstr {
    fn codegen(&self, ctx: &mut CodegenContext) -> Result<Bytecode, Box<dyn std::error::Error>> {
        match self {
            IRInstr::Cpy { src, dst } => {
                let op = src.load_op(ctx);

                let name = match dst {
                    IRVal::Var(name) => name,
                    IRVal::Const(_) => unreachable!(),
                };

                ctx.register::<String>(name);

                Ok(vec![op])
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

                let lhs = lhs.load_op(ctx);
                let rhs = rhs.load_op(ctx);

                let name = match dst {
                    IRVal::Var(name) => name,
                    IRVal::Const(_) => unreachable!(),
                };

                ctx.register::<String>(name);

                Ok(vec![lhs, rhs, op])
            }

            IRInstr::Ret { val } => match val {
                Some(v) => Ok(vec![v.load_op(ctx), Op::Ret]),
                None => Ok(vec![Op::RetVoid]),
            },

            IRInstr::Print { val, .. } => Ok(vec![val.load_op(ctx), Op::Print]),

            IRInstr::Jz { cond, target: _ } => Ok(vec![cond.load_op(ctx), Op::Jz(usize::MAX)]),

            IRInstr::Jmp { target: _ } => Ok(vec![Op::Jmp(usize::MAX)]),

            IRInstr::Label(_target) => Ok(vec![]),

            IRInstr::Call { target, args, dst } => {
                let mut ops = vec![];

                for arg in args {
                    ops.push(arg.load_op(ctx));
                }

                let idx = ctx.get::<BytecodeFunc>(target);

                if idx.is_none() {
                    return Err(IRError {
                        msg: format!("trying to call an undefined fn: {}", target),
                    }
                    .into());
                }

                ops.push(Op::Call {
                    target: idx.unwrap(),
                    argc: args.len(),
                });

                let dst_name = match dst {
                    IRVal::Var(name) => name,
                    IRVal::Const(_) => unreachable!(),
                };

                ctx.register::<String>(dst_name);

                Ok(ops)
            }
        }
    }
}

impl IRVal {
    fn load_op<U: Ctx>(&self, ctx: &mut U) -> Op {
        match self {
            IRVal::Const(konst) => Op::Const(*konst),
            IRVal::Var(var) => {
                if let Some(idx) = ctx.get::<String>(var) {
                    Op::Deepget(idx)
                } else {
                    unreachable!();
                }
            }
        }
    }

    fn store_op<U: Ctx>(&self, ctx: &U) -> Op {
        match self {
            IRVal::Const(_konst) => unreachable!(),
            IRVal::Var(var) => {
                if let Some(idx) = ctx.get::<String>(var) {
                    Op::Deepset(idx)
                } else {
                    unreachable!();
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct IRError {
    pub msg: String,
}

impl std::error::Error for IRError {}

impl std::fmt::Display for IRError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ir: {}", self.msg)
    }
}
