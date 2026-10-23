use crate::{
    bytecode::r#gen::BytecodeFunc,
    ir::r#gen::{Codegen, CodegenContext, Ctx, IRFunc, IRInstr, IRProg, IRVal},
    parser::ast::{BinaryExpressionKind, Literal, Type},
    runtime::{
        jit::JIT_RET,
        rt::Rt,
        value::{Value, ValueTag},
    },
};

#[derive(Debug)]
pub struct AsmProg {
    pub funcs: Vec<AsmFunc>,
}

#[derive(Debug)]
pub struct AsmFunc {
    pub name: String,
    pub body: Vec<AsmInstr>,
    pub space: usize,
}

#[derive(Debug, Clone)]
pub enum AsmInstr {
    Mov {
        src: AsmOp,
        dst: AsmOp,
    },
    Ret,
    Push(AsmOp),
    Pop(AsmOp),
    Bin {
        kind: AsmInstrBinKind,
        lhs: AsmOp,
        rhs: AsmOp,
    },
    Cmp {
        src: AsmOp,
        dst: AsmOp,
    },
    Jz {
        target: String,
    },
    Jnz {
        target: String,
    },
    Jmp {
        target: String,
    },
    Label(String),
    Call {
        target: String,
    },
    CallRt {
        target: RtFn,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum RtFn {
    Print,
    Call,
}

impl RtFn {
    pub fn offset(self) -> usize {
        match self {
            RtFn::Print => std::mem::offset_of!(Rt, print),
            RtFn::Call => std::mem::offset_of!(Rt, call),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AsmInstrBinKind {
    Add,
    Sub,
}

#[derive(Debug, Clone)]
pub enum AsmOp {
    Pseudo(String),
    Reg(AsmReg),
    Imm(i64),
    Stack(isize),
    Mem(AsmReg, isize),
}

#[derive(Debug, Clone, Copy)]
pub enum AsmReg {
    Bp,
    Sp,
    Ax,
    R8,
    R9,
    R10,
    R11,
    R12,
    R13,
    R14,
    Cx,
    Dx,
    Di,
    Si,
}

impl Codegen<AsmProg, CodegenContext> for IRProg {
    fn codegen(&self, ctx: &mut CodegenContext) -> Result<AsmProg, Box<dyn std::error::Error>> {
        Ok(AsmProg {
            funcs: self
                .funcs
                .iter()
                .map(|f| f.codegen(ctx))
                .collect::<Result<Vec<AsmFunc>, _>>()?,
        })
    }
}

impl Codegen<AsmFunc, CodegenContext> for IRFunc {
    fn codegen(&self, ctx: &mut CodegenContext) -> Result<AsmFunc, Box<dyn std::error::Error>> {
        let mut body = vec![];

        insert_prologue(&mut body);

        /* %rdi = *const Value  */
        for (idx, param) in self.params.iter().enumerate() {
            let offset = (idx * std::mem::size_of::<Value>() + std::mem::offset_of!(Value, payload))
                as isize;

            body.push(AsmInstr::Mov {
                src: AsmOp::Mem(AsmReg::Di, offset),
                dst: AsmOp::Pseudo(param.name.clone()),
            });
        }

        for instr in &self.body {
            body.extend(instr.codegen_with(ctx, self.ret_ty, &self.name)?);
        }

        Ok(AsmFunc {
            name: self.name.to_owned(),
            body,
            space: 0,
        })
    }
}

impl IRInstr {
    fn codegen_with(
        &self,
        ctx: &mut CodegenContext,
        ret_ty: Option<Type>,
        func_name: &str,
    ) -> Result<Vec<AsmInstr>, Box<dyn std::error::Error>> {
        match self {
            IRInstr::Cpy { src, dst } => Ok(vec![AsmInstr::Mov {
                src: src.codegen(ctx)?,
                dst: dst.codegen(ctx)?,
            }]),

            IRInstr::Ret { val } => {
                let mut instrs = vec![
                    AsmInstr::Mov {
                        src: val
                            .as_ref()
                            .map(|val| val.codegen(ctx))
                            .transpose()?
                            .unwrap_or(AsmOp::Imm(0)),
                        dst: AsmOp::Reg(AsmReg::Ax),
                    },
                    AsmInstr::Mov {
                        src: AsmOp::Imm(ret_ty.map(ValueTag::from).unwrap_or(ValueTag::Void) as i64),
                        dst: AsmOp::Reg(AsmReg::Dx),
                    },
                    AsmInstr::Mov {
                        src: AsmOp::Imm(JIT_RET as i64),
                        dst: AsmOp::Reg(AsmReg::Cx),
                    },
                ];

                insert_epilogue(&mut instrs);
                instrs.push(AsmInstr::Ret);

                Ok(instrs)
            }

            IRInstr::Bin {
                kind,
                lhs,
                rhs,
                dst,
            } => {
                let kind = match kind {
                    BinaryExpressionKind::Add => AsmInstrBinKind::Add,
                    BinaryExpressionKind::Sub => AsmInstrBinKind::Sub,
                    _ => unimplemented!(),
                };

                let instrs = vec![
                    AsmInstr::Mov {
                        src: lhs.codegen(ctx)?,
                        dst: dst.codegen(ctx)?,
                    },
                    AsmInstr::Bin {
                        kind,
                        lhs: rhs.codegen(ctx)?,
                        rhs: dst.codegen(ctx)?,
                    },
                ];

                Ok(instrs)
            }

            IRInstr::Print { val, ty } => {
                let payload = val.codegen(ctx)?;

                let tag = ValueTag::from(*ty) as u64;

                Ok(vec![
                    AsmInstr::Push(AsmOp::Reg(AsmReg::R8)),
                    AsmInstr::Push(AsmOp::Reg(AsmReg::R10)),
                    AsmInstr::Push(AsmOp::Reg(AsmReg::R11)),
                    AsmInstr::Bin {
                        kind: AsmInstrBinKind::Sub,
                        lhs: AsmOp::Imm(8),
                        rhs: AsmOp::Reg(AsmReg::Sp),
                    },
                    // *mut Vm
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::R13),
                        dst: AsmOp::Reg(AsmReg::Di),
                    },
                    // ValueTag
                    AsmInstr::Mov {
                        src: AsmOp::Imm(tag as i64),
                        dst: AsmOp::Reg(AsmReg::Si),
                    },
                    // payload
                    AsmInstr::Mov {
                        src: payload,
                        dst: AsmOp::Reg(AsmReg::Dx),
                    },
                    AsmInstr::CallRt {
                        target: RtFn::Print,
                    },
                    // restore stack
                    AsmInstr::Bin {
                        kind: AsmInstrBinKind::Add,
                        lhs: AsmOp::Imm(8),
                        rhs: AsmOp::Reg(AsmReg::Sp),
                    },
                    AsmInstr::Pop(AsmOp::Reg(AsmReg::R11)),
                    AsmInstr::Pop(AsmOp::Reg(AsmReg::R10)),
                    AsmInstr::Pop(AsmOp::Reg(AsmReg::R8)),
                    // check for error
                    AsmInstr::Cmp {
                        src: AsmOp::Imm(0),
                        dst: AsmOp::Reg(AsmReg::Ax),
                    },
                    AsmInstr::Jnz {
                        target: format!("{}_rt_err", func_name),
                    },
                ])
            }

            IRInstr::Jz { cond, target } => Ok(vec![
                AsmInstr::Cmp {
                    src: cond.codegen(ctx)?,
                    dst: AsmOp::Imm(0),
                },
                AsmInstr::Jz {
                    target: format!(".L{}_{}", func_name, target),
                },
            ]),

            IRInstr::Jmp { target } => Ok(vec![AsmInstr::Jmp {
                target: format!(".L{}_{}", func_name, target),
            }]),

            IRInstr::Label(target) => {
                Ok(vec![AsmInstr::Label(format!(".L{}_{}", func_name, target))])
            }

            IRInstr::Call { target, args, dst } => {
                let func = ctx.get::<BytecodeFunc>(target).unwrap();

                let argc = args.len();
                let args_space = argc * 8;

                let frame_space = (args_space + 16).div_ceil(16) * 16;

                let mut body = vec![
                    AsmInstr::Push(AsmOp::Reg(AsmReg::R8)),
                    AsmInstr::Push(AsmOp::Reg(AsmReg::R10)),
                    AsmInstr::Push(AsmOp::Reg(AsmReg::R11)),
                    /* FIXME: don't hardcode this */
                    /* Fix alignment after the three pushes. */
                    AsmInstr::Bin {
                        kind: AsmInstrBinKind::Sub,
                        lhs: AsmOp::Imm(8),
                        rhs: AsmOp::Reg(AsmReg::Sp),
                    },
                    /* Alloc args and retval space.  */
                    AsmInstr::Bin {
                        kind: AsmInstrBinKind::Sub,
                        lhs: AsmOp::Imm(frame_space as i64),
                        rhs: AsmOp::Reg(AsmReg::Sp),
                    },
                ];

                /* Materialize each arg on the native stack.  */
                for (idx, arg) in args.iter().enumerate() {
                    body.extend([
                        AsmInstr::Mov {
                            src: arg.codegen(ctx)?,
                            dst: AsmOp::Reg(AsmReg::R9),
                        },
                        AsmInstr::Mov {
                            src: AsmOp::Reg(AsmReg::R9),
                            dst: AsmOp::Mem(AsmReg::Sp, (idx * 8) as isize),
                        },
                    ]);
                }

                body.extend([
                    /* %rdi = *mut Vm,  */
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::R13),
                        dst: AsmOp::Reg(AsmReg::Di),
                    },
                    /* %rsi = fn idx  */
                    AsmInstr::Mov {
                        src: AsmOp::Imm(func as i64),
                        dst: AsmOp::Reg(AsmReg::Si),
                    },
                    /* %rdx = *const u64 (ptr to args)  */
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::Sp),
                        dst: AsmOp::Reg(AsmReg::Dx),
                    },
                    /* %rcx = argc  */
                    AsmInstr::Mov {
                        src: AsmOp::Imm(argc as i64),
                        dst: AsmOp::Reg(AsmReg::Cx),
                    },
                    /* %r8 = ptr to retval  */
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::Sp),
                        dst: AsmOp::Reg(AsmReg::R8),
                    },
                    AsmInstr::Bin {
                        kind: AsmInstrBinKind::Add,
                        lhs: AsmOp::Imm(args_space as i64),
                        rhs: AsmOp::Reg(AsmReg::R8),
                    },
                    /* invoke the rt trampoline  */
                    AsmInstr::CallRt { target: RtFn::Call },
                    /* read in Value.payload into %r9 scratch  */
                    AsmInstr::Mov {
                        src: AsmOp::Mem(AsmReg::Sp, (args_space + 8) as isize),
                        dst: AsmOp::Reg(AsmReg::R9),
                    },
                    /* release tmp space  */
                    AsmInstr::Bin {
                        kind: AsmInstrBinKind::Add,
                        lhs: AsmOp::Imm(frame_space as i64),
                        rhs: AsmOp::Reg(AsmReg::Sp),
                    },
                    /* rm alignment padding */
                    AsmInstr::Bin {
                        kind: AsmInstrBinKind::Add,
                        lhs: AsmOp::Imm(8),
                        rhs: AsmOp::Reg(AsmReg::Sp),
                    },
                    /* restore caller-saved regs  */
                    AsmInstr::Pop(AsmOp::Reg(AsmReg::R11)),
                    AsmInstr::Pop(AsmOp::Reg(AsmReg::R10)),
                    AsmInstr::Pop(AsmOp::Reg(AsmReg::R8)),
                    /* check rt status  */
                    AsmInstr::Cmp {
                        src: AsmOp::Imm(0),
                        dst: AsmOp::Reg(AsmReg::Ax),
                    },
                    AsmInstr::Jnz {
                        target: format!("{}_rt_err", func_name),
                    },
                ]);

                /* store the result into IR dst  */
                body.push(AsmInstr::Mov {
                    src: AsmOp::Reg(AsmReg::R9),
                    dst: dst.codegen(ctx)?,
                });

                Ok(body)
            }
        }
    }
}

impl Codegen<AsmOp, CodegenContext> for IRVal {
    fn codegen(&self, _ctx: &mut CodegenContext) -> Result<AsmOp, Box<dyn std::error::Error>> {
        match self {
            IRVal::Const(konst) => match konst {
                Literal::UInt8(uint8) => Ok(AsmOp::Imm(*uint8 as i64)),
                Literal::Int8(int8) => Ok(AsmOp::Imm(*int8 as i64)),
                Literal::UInt16(uint16) => Ok(AsmOp::Imm(*uint16 as i64)),
                Literal::Int16(int16) => Ok(AsmOp::Imm(*int16 as i64)),
                Literal::UInt32(uint32) => Ok(AsmOp::Imm(*uint32 as i64)),
                Literal::Int32(int32) => Ok(AsmOp::Imm(*int32 as i64)),
                Literal::UInt64(uint64) => Ok(AsmOp::Imm(*uint64 as i64)),
                Literal::Int64(int64) => Ok(AsmOp::Imm(*int64)),
                Literal::Bool(boolean) => Ok(AsmOp::Imm(if *boolean { 1 } else { 0 })),
                _ => unimplemented!(),
            },
            IRVal::Var(var) => Ok(AsmOp::Pseudo(var.to_owned())),
        }
    }
}

pub trait AsmSink {
    fn sink(&mut self, instr: AsmInstr);
}

impl AsmSink for Vec<AsmInstr> {
    fn sink(&mut self, instr: AsmInstr) {
        self.push(instr);
    }
}

impl AsmSink for String {
    fn sink(&mut self, instr: AsmInstr) {
        self.push_str(&instr.to_string());
    }
}

pub fn insert_prologue<T>(body: &mut T)
where
    T: AsmSink,
{
    body.sink(AsmInstr::Push(AsmOp::Reg(AsmReg::Bp)));
    body.sink(AsmInstr::Mov {
        src: AsmOp::Reg(AsmReg::Sp),
        dst: AsmOp::Reg(AsmReg::Bp),
    });
    body.sink(AsmInstr::Bin {
        kind: AsmInstrBinKind::Sub,
        lhs: AsmOp::Imm(0),
        rhs: AsmOp::Reg(AsmReg::Sp),
    });
}

pub fn insert_epilogue<T>(body: &mut T)
where
    T: AsmSink,
{
    body.sink(AsmInstr::Mov {
        src: AsmOp::Reg(AsmReg::Bp),
        dst: AsmOp::Reg(AsmReg::Sp),
    });
    body.sink(AsmInstr::Pop(AsmOp::Reg(AsmReg::Bp)));
}
