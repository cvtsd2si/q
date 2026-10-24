use crate::{
    bytecode::r#gen::BytecodeFunc,
    ir::r#gen::{Codegen, CodegenContext, Ctx, IRFunc, IRInstr, IRProg, IRVal, mktmp},
    parser::ast::{BinaryExpressionKind, Literal, Type},
    runtime::{
        jit::{JIT_ERROR, JIT_RET},
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
    Jmpcc {
        cc: ConditionCode,
        target: String,
    },
    Setcc {
        cc: ConditionCode,
        dst: AsmOp,
    },
    Jmp {
        target: String,
    },
    Label(String),
    Call {
        target: String,
    },
    CallIndirect {
        target: AsmReg,
    },
    CallRt {
        target: RtFn,
    },
    Comment(String),
    Newline,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ConditionCode {
    E,
    Ne,

    // signed
    L,
    Le,
    G,
    Ge,

    // unsigned
    B,
    Be,
    A,
    Ae,
}

impl std::fmt::Display for ConditionCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConditionCode::E => write!(f, "e"),
            ConditionCode::Ne => write!(f, "ne"),
            ConditionCode::L => write!(f, "l"),
            ConditionCode::Le => write!(f, "le"),
            ConditionCode::G => write!(f, "g"),
            ConditionCode::Ge => write!(f, "ge"),
            ConditionCode::B => write!(f, "b"),
            ConditionCode::Be => write!(f, "be"),
            ConditionCode::A => write!(f, "a"),
            ConditionCode::Ae => write!(f, "ae"),
        }
    }
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
    R15,
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
                operand_ty,
            } if matches!(kind, BinaryExpressionKind::Add | BinaryExpressionKind::Sub) => {
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

            IRInstr::Bin {
                kind,
                lhs,
                rhs,
                dst,
                operand_ty,
            } => {
                let is_unsigned = matches!(
                    *operand_ty,
                    Type::UInt8 | Type::UInt16 | Type::UInt32 | Type::UInt64
                );

                let cc = match (*kind, is_unsigned) {
                    (BinaryExpressionKind::Equal, _) => ConditionCode::E,
                    (BinaryExpressionKind::NotEqual, _) => ConditionCode::Ne,
                    (BinaryExpressionKind::Less, false) => ConditionCode::L,
                    (BinaryExpressionKind::LessEqual, false) => ConditionCode::Le,
                    (BinaryExpressionKind::Greater, false) => ConditionCode::G,
                    (BinaryExpressionKind::GreaterEqual, false) => ConditionCode::Ge,
                    (BinaryExpressionKind::Less, true) => ConditionCode::B,
                    (BinaryExpressionKind::LessEqual, true) => ConditionCode::Be,
                    (BinaryExpressionKind::Greater, true) => ConditionCode::A,
                    (BinaryExpressionKind::GreaterEqual, true) => ConditionCode::Ae,
                    _ => unreachable!("got: {kind:?} {is_unsigned:?}"),
                };

                let instrs = vec![
                    AsmInstr::Cmp {
                        src: rhs.codegen(ctx)?,
                        dst: lhs.codegen(ctx)?,
                    },
                    AsmInstr::Mov {
                        src: AsmOp::Imm(0),
                        dst: dst.codegen(ctx)?,
                    },
                    AsmInstr::Setcc {
                        cc,
                        dst: dst.codegen(ctx)?,
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
                    AsmInstr::Jmpcc {
                        cc: ConditionCode::Ne,
                        target: format!("{}_rt_err", func_name),
                    },
                ])
            }

            IRInstr::Jz { cond, target } => Ok(vec![
                AsmInstr::Cmp {
                    src: cond.codegen(ctx)?,
                    dst: AsmOp::Imm(0),
                },
                AsmInstr::Jmpcc {
                    cc: ConditionCode::E,
                    target: format!(".L{}_{}", func_name, target),
                },
            ]),

            IRInstr::Jmp { target } => Ok(vec![AsmInstr::Jmp {
                target: format!(".L{}_{}", func_name, target),
            }]),

            IRInstr::Label(target) => {
                Ok(vec![AsmInstr::Label(format!(".L{}_{}", func_name, target))])
            }

            IRInstr::Call {
                target,
                args,
                arg_tys,
                dst,
            } => {
                let func = ctx.get::<BytecodeFunc>(target).unwrap();

                let argc = args.len();
                let args_space = argc * 8;

                let native_args_offset = args_space + std::mem::size_of::<Value>();
                let native_args_space = argc * std::mem::size_of::<Value>();

                let frame_space = (native_args_offset + native_args_space).div_ceil(16) * 16;

                let mut body = vec![
                    AsmInstr::Comment(format!("Call `{target}`.")),
                    AsmInstr::Comment("Preserve caller saved registers.".to_string()),
                    AsmInstr::Push(AsmOp::Reg(AsmReg::R8)),
                    AsmInstr::Push(AsmOp::Reg(AsmReg::R10)),
                    AsmInstr::Push(AsmOp::Reg(AsmReg::R11)),
                    AsmInstr::Newline,
                    /* FIXME: don't hardcode this */
                    AsmInstr::Comment("Fix alignment after the three pushes.".to_string()),
                    AsmInstr::Bin {
                        kind: AsmInstrBinKind::Sub,
                        lhs: AsmOp::Imm(8),
                        rhs: AsmOp::Reg(AsmReg::Sp),
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment(format!(
                        "Reserve {frame_space} bytes for call args ({argc} args) and retval."
                    )),
                    AsmInstr::Comment("Layout:".into()),
                    AsmInstr::Comment(format!("- {args_space} bytes of raw arguments,")),
                    AsmInstr::Comment(format!(
                        "- {} bytes for the runtime return Value,",
                        std::mem::size_of::<Value>()
                    )),
                    AsmInstr::Comment(format!(
                        "- {native_args_space} bytes of native Value arguments"
                    )),
                    AsmInstr::Comment("All this space is rounded up to 16-byte alignment".into()),
                    AsmInstr::Bin {
                        kind: AsmInstrBinKind::Sub,
                        lhs: AsmOp::Imm(frame_space as i64),
                        rhs: AsmOp::Reg(AsmReg::Sp),
                    },
                    AsmInstr::Newline,
                ];

                body.extend([AsmInstr::Comment(
                    "Prepare raw u64 args and tagged Value args.".into(),
                )]);

                /* Materialize each arg on the native stack.  */
                for (idx, arg) in args.iter().enumerate() {
                    body.extend([
                        AsmInstr::Comment(format!("Materializing arg {idx} on the native stack.")),
                        AsmInstr::Mov {
                            src: arg.codegen(ctx)?,
                            dst: AsmOp::Reg(AsmReg::R9),
                        },
                        AsmInstr::Mov {
                            src: AsmOp::Reg(AsmReg::R9),
                            dst: AsmOp::Mem(AsmReg::Sp, (idx * 8) as isize),
                        },
                        AsmInstr::Newline,
                    ]);

                    let base = native_args_offset + idx * std::mem::size_of::<Value>();
                    let tag = ValueTag::from(arg_tys[idx]);

                    body.extend([
                        AsmInstr::Comment(format!(
                            "Construct Value for arg {idx}: tag={tag:?}, payload follows."
                        )),
                        AsmInstr::Mov {
                            src: AsmOp::Imm(tag as i64),
                            dst: AsmOp::Mem(
                                AsmReg::Sp,
                                (base + std::mem::offset_of!(Value, tag)) as isize,
                            ),
                        },
                        AsmInstr::Mov {
                            src: AsmOp::Reg(AsmReg::R9),
                            dst: AsmOp::Mem(
                                AsmReg::Sp,
                                (base + std::mem::offset_of!(Value, payload)) as isize,
                            ),
                        },
                        AsmInstr::Newline,
                    ]);
                }

                let id = mktmp();

                let fallback_label = format!(".L{func_name}_call_fallback_{id}");
                let done_label = format!(".L{func_name}_call_done_{id}");

                body.extend([
                    AsmInstr::Comment(format!(
                        "Determine whether `{target}` has already been jitted." 
                    )),
                    AsmInstr::Comment(
                        "%r15 = `native_entrypoints`".into()
                    ),
                    AsmInstr::Comment(format!(
                        "Load `native_entrypoints[{func}]` from offset {} into %r9.",
                        func * std::mem::size_of::<usize>()
                    )),
                    AsmInstr::Comment(
                        "A zero entry means the function has not been jitted.".into(),
                    ),
                    AsmInstr::Comment(
                        "A nonzero entry is its code address.".into()
                    ),
                    AsmInstr::Mov {
                        src: AsmOp::Mem(
                            AsmReg::R15,
                            (func * std::mem::size_of::<usize>()) as isize,
                        ),
                        dst: AsmOp::Reg(AsmReg::R9),
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment(
                        "Compare the entrypoint against zero.".into(),
                    ),
                    AsmInstr::Comment(
                        "cmp updates the cpu flags with no changes to %r9.".into(),
                    ),
                    AsmInstr::Comment(
                        "If %r9 == 0, the Zero Flag (ZF) is set.".into()
                    ),
                    AsmInstr::Cmp {
                        src: AsmOp::Imm(0),
                        dst: AsmOp::Reg(AsmReg::R9),
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment(
                        "If ZF is set, jump to the Vm fallback and exec the uncompiled function.".into()
                    ),
                    AsmInstr::Comment(
                        "Otherwise, fall through to the native call path, with %r9 holding the fn addr.".into()
                    ),
                    AsmInstr::Jmpcc {
                        cc: ConditionCode::E,
                        target: fallback_label.clone(),
                    },
                    AsmInstr::Newline,
                ]);

                body.extend([
                    AsmInstr::Comment(
                        "The internal ABI expects %rdi to point to an array of Value objects."
                            .into(),
                    ),
                    AsmInstr::Comment(format!(
                        "This array has already been constructed on the stack, starting at {native_args_offset}(%rsp)."
                    )),
                    AsmInstr::Comment("Compute its address and pass that pointer in %rdi.".into()),
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::Sp),
                        dst: AsmOp::Reg(AsmReg::Di),
                    },
                    AsmInstr::Bin {
                        kind: AsmInstrBinKind::Add,
                        lhs: AsmOp::Imm(native_args_offset as i64),
                        rhs: AsmOp::Reg(AsmReg::Di),
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment(format!(
                        "Call the native implementation of `{target}` directly."
                    )),
                    AsmInstr::Comment(
                        "%r9 holds the entrypoint address loaded from `native_entrypoints`.".into(),
                    ),
                    AsmInstr::Comment(
                        "The callee receives its arguments through %rdi and inherits the".into()
                    ),
                    AsmInstr::Comment(
                        "JIT context in %r13 (Vm), %r14 (Rt), and %r15 (`native entrypoints`)."
                            .into(),
                    ),
                    AsmInstr::Comment(
                        "On return: %rax = payload or err code, %rdx = ValueTag, %rcx = JIT exit kind."
                            .into(),
                    ),
                    AsmInstr::CallIndirect { target: AsmReg::R9 },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Check whether the callee returned JIT_ERROR.".into()),
                    AsmInstr::Comment(
                        "cmp sets ZF if %rcx == JIT_ERROR; je then branches to our error handler."
                            .into(),
                    ),
                    AsmInstr::Comment(
                        "On error, %rax still contains the error code.  The handler propagates".into(),
                    ),
                    AsmInstr::Comment(
                        "it to our caller and restores the stack frame."
                            .into(),
                    ),
                    AsmInstr::Cmp {
                        src: AsmOp::Imm(JIT_ERROR as i64),
                        dst: AsmOp::Reg(AsmReg::Cx),
                    },
                    AsmInstr::Jmpcc {
                        cc: ConditionCode::E,
                        target: format!("{func_name}_rt_err"),
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment(
                        "The native call succeeded.  The return payload is currently in %rax."
                            .into(),
                    ),
                    AsmInstr::Comment(
                        "The shared continuation expects the result in %r9 and a runtime".into(),
                    ),
                    AsmInstr::Comment(
                        "status code in %rax, because that is how jit_call() returns."
                            .into(),
                    ),
                    AsmInstr::Comment(
                        "Move the payload into %r9 before overwriting %rax with 0 (success)."
                            .into(),
                    ),
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::Ax),
                        dst: AsmOp::Reg(AsmReg::R9),
                    },
                    AsmInstr::Mov {
                        src: AsmOp::Imm(0),
                        dst: AsmOp::Reg(AsmReg::Ax),
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment(
                        "Skip the Vm fallback and continue at the shared stack-cleanup code."
                            .into(),
                    ),
                    AsmInstr::Jmp {
                        target: done_label.clone(),
                    },
                    AsmInstr::Newline,
                ]);

                body.push(AsmInstr::Label(fallback_label));
                body.push(AsmInstr::Comment(
                    "Native entrypoint unavailable; call through the Vm".into(),
                ));
                body.extend([
                    AsmInstr::Comment("%rdi = *mut Vm".into()),
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::R13),
                        dst: AsmOp::Reg(AsmReg::Di),
                    },
                    AsmInstr::Comment("%rsi = fn idx".into()),
                    AsmInstr::Mov {
                        src: AsmOp::Imm(func as i64),
                        dst: AsmOp::Reg(AsmReg::Si),
                    },
                    AsmInstr::Comment("%rdx = *const u64 (ptr to args)".into()),
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::Sp),
                        dst: AsmOp::Reg(AsmReg::Dx),
                    },
                    AsmInstr::Comment("%rcx = argc".into()),
                    AsmInstr::Mov {
                        src: AsmOp::Imm(argc as i64),
                        dst: AsmOp::Reg(AsmReg::Cx),
                    },
                    AsmInstr::Comment("%r8 = ptr to retval".into()),
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::Sp),
                        dst: AsmOp::Reg(AsmReg::R8),
                    },
                    AsmInstr::Comment("Reserve space for the args.".into()),
                    AsmInstr::Bin {
                        kind: AsmInstrBinKind::Add,
                        lhs: AsmOp::Imm(args_space as i64),
                        rhs: AsmOp::Reg(AsmReg::R8),
                    },
                    AsmInstr::Comment("Invoke the rt trampoline".into()),
                    AsmInstr::CallRt { target: RtFn::Call },
                    AsmInstr::Comment("Read in Value.payload into %r9 scratch.".into()),
                    AsmInstr::Mov {
                        src: AsmOp::Mem(AsmReg::Sp, (args_space + 8) as isize),
                        dst: AsmOp::Reg(AsmReg::R9),
                    },
                    AsmInstr::Comment("Both native and fallback calls keep on going here.".into()),
                    AsmInstr::Label(done_label),
                    AsmInstr::Comment("Release tmp space.".into()),
                    AsmInstr::Bin {
                        kind: AsmInstrBinKind::Add,
                        lhs: AsmOp::Imm(frame_space as i64),
                        rhs: AsmOp::Reg(AsmReg::Sp),
                    },
                    AsmInstr::Comment("Release alignment padding.".into()),
                    AsmInstr::Bin {
                        kind: AsmInstrBinKind::Add,
                        lhs: AsmOp::Imm(8),
                        rhs: AsmOp::Reg(AsmReg::Sp),
                    },
                    AsmInstr::Comment("Restore caller-saved regs.".into()),
                    AsmInstr::Pop(AsmOp::Reg(AsmReg::R11)),
                    AsmInstr::Pop(AsmOp::Reg(AsmReg::R10)),
                    AsmInstr::Pop(AsmOp::Reg(AsmReg::R8)),
                    AsmInstr::Comment("Check status.".into()),
                    AsmInstr::Cmp {
                        src: AsmOp::Imm(0),
                        dst: AsmOp::Reg(AsmReg::Ax),
                    },
                    AsmInstr::Jmpcc {
                        cc: ConditionCode::Ne,
                        target: format!("{}_rt_err", func_name),
                    },
                ]);

                body.extend([
                    AsmInstr::Comment("Store the result into IR dst.".into()),
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::R9),
                        dst: dst.codegen(ctx)?,
                    },
                ]);

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
    body.sink(AsmInstr::Comment("Establish a stack frame".to_string()));
    body.sink(AsmInstr::Push(AsmOp::Reg(AsmReg::Bp)));
    body.sink(AsmInstr::Mov {
        src: AsmOp::Reg(AsmReg::Sp),
        dst: AsmOp::Reg(AsmReg::Bp),
    });
    body.sink(AsmInstr::Newline);
    body.sink(AsmInstr::Comment("Reserve the stack space.".to_string()));
    body.sink(AsmInstr::Bin {
        kind: AsmInstrBinKind::Sub,
        lhs: AsmOp::Imm(0),
        rhs: AsmOp::Reg(AsmReg::Sp),
    });
    body.sink(AsmInstr::Newline);
}

pub fn insert_epilogue<T>(body: &mut T)
where
    T: AsmSink,
{
    body.sink(AsmInstr::Comment(
        "Restore the previous stack frame".to_string(),
    ));
    body.sink(AsmInstr::Mov {
        src: AsmOp::Reg(AsmReg::Bp),
        dst: AsmOp::Reg(AsmReg::Sp),
    });
    body.sink(AsmInstr::Pop(AsmOp::Reg(AsmReg::Bp)));
    body.sink(AsmInstr::Newline);
}
