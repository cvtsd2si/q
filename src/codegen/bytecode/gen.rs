use std::collections::HashMap;

use crate::{
    QError,
    codegen::{
        Codegen, CodegenContext,
        ir::r#gen::{IRFunc, IRInstr, IRProg, IRVal},
    },
    parser::ast::{BinaryExpressionKind, Literal, Type, UnaryExpressionKind},
    tokenizer::token::Span,
};

#[derive(Debug, Clone, Copy)]
pub enum Op {
    Const(Literal),
    StrConst(usize),
    Unary(UnaryExpressionKind),
    Binary(BinaryExpressionKind),
    Ret,
    RetVoid,
    Print,
    Deepget(usize),
    Deepset(usize),
    Jz(usize),
    Jmp(usize),
    Call {
        target: usize,
        argc: usize,
    },
    Cvt {
        src_ty: Type,
        dst_ty: Type,
    },
    GetAddr {
        local: usize,
        ty: Type,
    },
    Load(Type),
    Store(Type),
    AddPtr {
        scale: usize,
    },
    CpyToOffset {
        local: usize,
        offset: usize,
        ty: Type,
    },
    CpyFromOffset {
        local: usize,
        offset: usize,
        ty: Type,
    },
}

pub type Bytecode = Vec<Op>;

#[derive(Debug)]
pub struct BytecodeProg {
    pub funcs: Vec<BytecodeFunc>,
    pub strings: Vec<String>,
}

#[derive(Debug)]
pub struct BytecodeFunc {
    pub name: String,
    pub body: Bytecode,
    pub locals_count: usize,
    pub ir_map: Vec<usize>,
    pub aggregates: HashMap<usize, AggregateLocal>,
}

#[derive(Debug, Clone, Copy)]
pub struct AggregateLocal {
    pub slot: usize,
    pub size: usize,
}

impl Codegen<BytecodeProg> for IRProg {
    fn codegen(&self, ctx: &mut CodegenContext) -> Result<BytecodeProg, QError> {
        let mut bytecode_prog = BytecodeProg {
            funcs: vec![],
            strings: vec![],
        };

        for f in self.functions() {
            ctx.register::<BytecodeFunc>(&f.name);
        }

        for f in self.functions() {
            bytecode_prog.funcs.push(f.codegen(ctx)?);
        }

        bytecode_prog.strings = std::mem::take(&mut ctx.string_pool);
        ctx.string_indexes.clear();

        Ok(bytecode_prog)
    }
}

impl Codegen<BytecodeFunc> for IRFunc {
    fn codegen(&self, ctx: &mut CodegenContext) -> Result<BytecodeFunc, QError> {
        ctx.locals.clear();

        for param in &self.params {
            ctx.register::<String>(&param.name);
        }

        let mut aggregates = HashMap::new();

        for instr in &self.body {
            if let Some(IRVal::Var { name, .. }) = instr.dst() {
                ctx.register::<String>(name);
            }

            let aggregate = match instr {
                IRInstr::GetAddr { src, .. } | IRInstr::CpyFromOffset { src, .. } => Some(src),
                IRInstr::CpyToOffset { dst, .. } => Some(dst),
                _ => None,
            };

            if let Some(IRVal::Var {
                name,
                ty: Type::Array(id),
            }) = aggregate
            {
                let slot = ctx.register::<String>(name);

                aggregates.insert(
                    slot,
                    AggregateLocal {
                        slot,
                        size: Type::Array(*id).size_of(ctx.types),
                    },
                );
            }
        }

        let mut f = BytecodeFunc {
            name: self.name.to_owned(),
            body: vec![],
            locals_count: ctx.locals.len(),
            ir_map: vec![],
            aggregates,
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

impl Codegen<Bytecode> for IRInstr {
    fn codegen(&self, ctx: &mut CodegenContext) -> Result<Bytecode, QError> {
        match self {
            IRInstr::Cpy { src, src_ty, dst } => {
                let mut ops = vec![src.load_op(ctx)];

                let (name, dst_ty) = match dst {
                    IRVal::Var { name, ty } => (name, ty),
                    _ => unreachable!(),
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
                let src = src.load_op(ctx);

                let name = match dst {
                    IRVal::Var { name, .. } => name,
                    _ => unreachable!(),
                };

                ctx.register::<String>(name);

                Ok(vec![src, Op::Unary(*kind), dst.store_op(ctx)])
            }

            IRInstr::Bin {
                kind,
                lhs,
                rhs,
                dst,
                ..
            } => {
                let name = match dst {
                    IRVal::Var { name, .. } => name,
                    _ => unreachable!(),
                };

                ctx.register::<String>(name);

                let mut ops = vec![lhs.load_op(ctx), rhs.load_op(ctx), Op::Binary(*kind)];

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
                    return Err(QError {
                        kind: crate::QErrorKind::Bytecode,
                        code: 1,
                        msg: format!("trying to call an undefined fn: {}", target),
                        span: Span { start: 0, end: 0 },
                    });
                }

                ops.push(Op::Call {
                    target: idx.unwrap(),
                    argc: args.len(),
                });

                let dst_name = match dst {
                    IRVal::Var { name, .. } => name,
                    _ => unreachable!(),
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

            IRInstr::Load { src_ptr, dst } => {
                let ty = dst.ty();

                Ok(vec![src_ptr.load_op(ctx), Op::Load(ty), dst.store_op(ctx)])
            }

            IRInstr::Store { src, dst_ptr } => {
                let Type::Ptr(id) = dst_ptr.ty() else {
                    unreachable!("Store requires a pointer");
                };

                let ty = ctx.types.get_pointer(id);

                Ok(vec![dst_ptr.load_op(ctx), src.load_op(ctx), Op::Store(ty)])
            }

            IRInstr::AddPtr {
                ptr,
                index,
                scale,
                dst,
            } => Ok(vec![
                ptr.load_op(ctx),
                index.load_op(ctx),
                Op::AddPtr { scale: *scale },
                dst.store_op(ctx),
            ]),

            IRInstr::GetAddr { src, dst } => {
                let IRVal::Var { name, ty } = src else {
                    unreachable!("GetAddr expects a variable");
                };

                let slot = ctx.get::<String>(name).unwrap();

                Ok(vec![
                    Op::GetAddr {
                        local: slot,
                        ty: *ty,
                    },
                    dst.store_op(ctx),
                ])
            }

            IRInstr::CpyFromOffset { src, offset, dst } => {
                let IRVal::Var { name, .. } = src else {
                    unreachable!("CpyFromOffset expects a variable");
                };

                let slot = ctx.get::<String>(name).unwrap();

                Ok(vec![
                    Op::CpyFromOffset {
                        local: slot,
                        offset: *offset,
                        ty: dst.ty(),
                    },
                    dst.store_op(ctx),
                ])
            }

            IRInstr::CpyToOffset { src, dst, offset } => {
                let IRVal::Var { name, .. } = dst else {
                    unreachable!("CpyToOffset expects a variable");
                };

                let slot = ctx.get::<String>(name).unwrap();

                Ok(vec![
                    src.load_op(ctx),
                    Op::CpyToOffset {
                        local: slot,
                        offset: *offset,
                        ty: src.ty(),
                    },
                ])
            }
        }
    }
}

impl IRVal {
    fn load_op(&self, ctx: &mut CodegenContext) -> Op {
        match self {
            IRVal::Const(konst) => Op::Const(*konst),
            IRVal::Str(string) => Op::StrConst(ctx.intern_string(string)),
            IRVal::Var { name, .. } => {
                if let Some(idx) = ctx.get::<String>(name) {
                    Op::Deepget(idx)
                } else {
                    unreachable!();
                }
            }
        }
    }

    fn store_op(&self, ctx: &CodegenContext) -> Op {
        match self {
            IRVal::Const(_) | IRVal::Str(_) => unreachable!(),
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
