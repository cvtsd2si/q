use std::collections::HashMap;

use crate::{
    ir::r#gen::{Codegen, CodegenContext, Ctx, IRFunc, IRInstr, IRProg, IRVal},
    parser::ast::{BinaryExpressionKind, Literal, Type, UnaryExpressionKind},
};

#[derive(Debug, Clone, Copy)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Const(Literal),
    Ret,
    RetVoid,
    Print,
    Deepget(usize),
    Deepset(usize),
    Jz(usize),
    Jmp(usize),
    Call { target: usize, argc: usize },
    Lt,
    Le,
    Eq,
    Neg,
    Not,
    Cvt { src_ty: Type, dst_ty: Type },
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
    pub locals_count: usize,
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

        for instr in &self.body {
            if let Some(IRVal::Var { name, .. }) = instr.dst() {
                ctx.register::<String>(name);
            }
        }

        let mut f = BytecodeFunc {
            name: self.name.to_owned(),
            body: vec![],
            locals_count: ctx.locals.len(),
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
            IRInstr::Cpy { src, src_ty, dst } => {
                let mut ops = vec![src.load_op(ctx)];

                let (name, dst_ty) = match dst {
                    IRVal::Var { name, ty } => (name, ty),
                    IRVal::Const(_) => unreachable!(),
                };

                /* Change the dst ValueTag.  */
                if src_ty != dst_ty {
                    ops.push(Op::Cvt {
                        src_ty: *src_ty,
                        dst_ty: *dst_ty,
                    });
                }

                if ctx.get::<String>(name).is_some() {
                    ops.push(dst.store_op(ctx));
                } else {
                    ctx.register::<String>(name);
                }

                Ok(ops)
            }

            IRInstr::Unary { kind, src, dst, .. } => {
                let op = match kind {
                    UnaryExpressionKind::Negate => Op::Neg,
                    UnaryExpressionKind::LogicalNot | UnaryExpressionKind::BitwiseNot => Op::Not,
                };

                let src = src.load_op(ctx);

                let name = match dst {
                    IRVal::Var { name, .. } => name,
                    IRVal::Const(_) => unreachable!(),
                };

                ctx.register::<String>(name);

                Ok(vec![src, op, dst.store_op(ctx)])
            }

            IRInstr::Bin {
                kind,
                lhs,
                rhs,
                dst,
                ..
            } => {
                let (op, swap, negate) = match kind {
                    BinaryExpressionKind::Add => (Op::Add, false, false),
                    BinaryExpressionKind::Sub => (Op::Sub, false, false),
                    BinaryExpressionKind::Mul => (Op::Mul, false, false),
                    BinaryExpressionKind::Div => (Op::Div, false, false),
                    BinaryExpressionKind::Rem => (Op::Rem, false, false),
                    BinaryExpressionKind::Less => (Op::Lt, false, false),
                    BinaryExpressionKind::Greater => (Op::Lt, true, false),
                    BinaryExpressionKind::LessEqual => (Op::Le, false, false),
                    BinaryExpressionKind::GreaterEqual => (Op::Le, true, false),
                    BinaryExpressionKind::Equal => (Op::Eq, false, false),
                    BinaryExpressionKind::NotEqual => (Op::Eq, false, true),
                };

                let (lhs, rhs) = if swap { (rhs, lhs) } else { (lhs, rhs) };

                let name = match dst {
                    IRVal::Var { name, .. } => name,
                    IRVal::Const(_) => unreachable!(),
                };

                ctx.register::<String>(name);

                let mut ops = vec![lhs.load_op(ctx), rhs.load_op(ctx), op];

                if negate {
                    ops.push(Op::Not);
                }

                ops.push(dst.store_op(ctx));

                Ok(ops)
            }

            IRInstr::Ret { val, .. } => match val {
                Some(v) => Ok(vec![v.load_op(ctx), Op::Ret]),
                None => Ok(vec![Op::RetVoid]),
            },

            IRInstr::Print { val, .. } => Ok(vec![val.load_op(ctx), Op::Print]),

            IRInstr::Jz { cond, .. } => Ok(vec![cond.load_op(ctx), Op::Jz(usize::MAX)]),

            IRInstr::Jmp { .. } => Ok(vec![Op::Jmp(usize::MAX)]),

            IRInstr::Label(_) => Ok(vec![]),

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

                ops.push(dst.store_op(ctx));

                Ok(ops)
            }

            IRInstr::ZeroExtend {
                src,
                dst,
                src_ty,
                dst_ty,
            }
            | IRInstr::SignExtend {
                src,
                dst,
                src_ty,
                dst_ty,
            }
            | IRInstr::Truncate {
                src,
                dst,
                src_ty,
                dst_ty,
            }
            | IRInstr::FloatToInt {
                src,
                src_ty,
                dst,
                dst_ty,
            }
            | IRInstr::IntToFloat {
                src,
                src_ty,
                dst,
                dst_ty,
            }
            | IRInstr::DoubleToInt {
                src,
                dst,
                src_ty,
                dst_ty,
            }
            | IRInstr::IntToDouble {
                src,
                dst,
                src_ty,
                dst_ty,
            }
            | IRInstr::FloatToUInt {
                src,
                src_ty,
                dst,
                dst_ty,
            }
            | IRInstr::UIntToFloat {
                src,
                src_ty,
                dst,
                dst_ty,
            }
            | IRInstr::DoubleToUInt {
                src,
                src_ty,
                dst,
                dst_ty,
            }
            | IRInstr::UIntToDouble {
                src,
                src_ty,
                dst,
                dst_ty,
            }
            | IRInstr::DoubleToFloat {
                src,
                src_ty,
                dst,
                dst_ty,
            }
            | IRInstr::FloatToDouble {
                src,
                src_ty,
                dst,
                dst_ty,
            } => {
                let src = src.load_op(ctx);

                let IRVal::Var { name, .. } = dst else {
                    unreachable!()
                };

                ctx.register::<String>(name);

                Ok(vec![
                    src,
                    Op::Cvt {
                        src_ty: *src_ty,
                        dst_ty: *dst_ty,
                    },
                    dst.store_op(ctx),
                ])
            }
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
            IRVal::Const(_) => unreachable!(),
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
