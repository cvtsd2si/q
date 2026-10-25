use std::collections::HashMap;

use crate::{
    ir::r#gen::{Codegen, CodegenContext, Ctx, IRFunc, IRInstr, IRProg, IRVal},
    parser::ast::{BinaryExpressionKind, Literal, Type, UnaryExpressionKind},
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
    L,
    G,
    Le,
    Ge,
    E,
    Ne,
    Neg,
    Not,
    BitwiseNot,
    Trunc { src_ty: Type, dst_ty: Type },
    Sx { src_ty: Type, dst_ty: Type },
    Zx { src_ty: Type, dst_ty: Type },
    UIntToDouble { src_ty: Type },
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
            IRInstr::Cpy { src, dst, .. } => {
                let src = src.load_op(ctx);

                let name = match dst {
                    IRVal::Var { name, .. } => name,
                    IRVal::Const(_) => unreachable!(),
                };

                if ctx.get::<String>(name).is_some() {
                    Ok(vec![src, dst.store_op(ctx)])
                } else {
                    ctx.register::<String>(name);
                    Ok(vec![src])
                }
            }

            IRInstr::Unary {
                kind,
                src,
                dst,
                operand_ty: _,
            } => {
                let op = match kind {
                    UnaryExpressionKind::Negate => Op::Neg,
                    UnaryExpressionKind::LogicalNot => Op::Not,
                    UnaryExpressionKind::BitwiseNot => Op::BitwiseNot,
                };

                let src = src.load_op(ctx);

                let name = match dst {
                    IRVal::Var { name, .. } => name,
                    IRVal::Const(_) => unreachable!(),
                };

                ctx.register::<String>(name);

                Ok(vec![src, op])
            }

            IRInstr::Bin {
                kind,
                lhs,
                rhs,
                dst,
                ..
            } => {
                let op = match kind {
                    BinaryExpressionKind::Add => Op::Add,
                    BinaryExpressionKind::Sub => Op::Sub,
                    BinaryExpressionKind::Less => Op::L,
                    BinaryExpressionKind::Greater => Op::G,
                    BinaryExpressionKind::LessEqual => Op::Le,
                    BinaryExpressionKind::GreaterEqual => Op::Ge,
                    BinaryExpressionKind::Equal => Op::E,
                    BinaryExpressionKind::NotEqual => Op::Ne,
                    _ => unreachable!(),
                };

                let lhs = lhs.load_op(ctx);
                let rhs = rhs.load_op(ctx);

                let name = match dst {
                    IRVal::Var { name, .. } => name,
                    IRVal::Const(_) => unreachable!(),
                };

                ctx.register::<String>(name);

                Ok(vec![lhs, rhs, op])
            }

            IRInstr::Ret { val, ret_ty: _ } => match val {
                Some(v) => Ok(vec![v.load_op(ctx), Op::Ret]),
                None => Ok(vec![Op::RetVoid]),
            },

            IRInstr::Print { val, .. } => Ok(vec![val.load_op(ctx), Op::Print]),

            IRInstr::Jz { cond, target: _ } => Ok(vec![cond.load_op(ctx), Op::Jz(usize::MAX)]),

            IRInstr::Jmp { target: _ } => Ok(vec![Op::Jmp(usize::MAX)]),

            IRInstr::Label(_target) => Ok(vec![]),

            IRInstr::Call {
                target, args, dst, ..
            } => {
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
                    IRVal::Var { name, .. } => name,
                    IRVal::Const(_) => unreachable!(),
                };

                ctx.register::<String>(dst_name);

                Ok(ops)
            }

            IRInstr::Truncate {
                src,
                dst,
                src_ty,
                dst_ty,
            } => {
                let src = src.load_op(ctx);

                let IRVal::Var { name, .. } = dst else {
                    unreachable!()
                };

                ctx.register::<String>(name);

                Ok(vec![
                    src,
                    Op::Trunc {
                        src_ty: *src_ty,
                        dst_ty: *dst_ty,
                    },
                ])
            }
            IRInstr::SignExtend {
                src,
                dst,
                src_ty,
                dst_ty,
            } => {
                let src = src.load_op(ctx);

                let IRVal::Var { name, .. } = dst else {
                    unreachable!();
                };

                ctx.register::<String>(name);

                Ok(vec![
                    src,
                    Op::Sx {
                        src_ty: *src_ty,
                        dst_ty: *dst_ty,
                    },
                ])
            }

            IRInstr::ZeroExtend {
                src,
                dst,
                src_ty,
                dst_ty,
            } => {
                let src = src.load_op(ctx);

                let IRVal::Var { name, .. } = dst else {
                    unreachable!();
                };

                ctx.register::<String>(name);

                Ok(vec![
                    src,
                    Op::Zx {
                        src_ty: *src_ty,
                        dst_ty: *dst_ty,
                    },
                ])
            }

            IRInstr::UIntToDouble {
                src, dst, src_ty, ..
            } => {
                let src = src.load_op(ctx);

                let IRVal::Var { name, .. } = dst else {
                    unreachable!();
                };

                ctx.register::<String>(name);

                Ok(vec![src, Op::UIntToDouble { src_ty: *src_ty }])
            }

            _ => todo!(),
        }
    }
}

impl IRVal {
    fn load_op<U: Ctx>(&self, ctx: &mut U) -> Op {
        match self {
            IRVal::Const(konst) => Op::Const(*konst),
            IRVal::Var { name, .. } => {
                if let Some(idx) = ctx.get::<String>(name) {
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
            IRVal::Var { name, .. } => {
                if let Some(idx) = ctx.get::<String>(name) {
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
