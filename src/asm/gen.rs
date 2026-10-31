use std::collections::HashMap;

use crate::{
    asm::abi::{
        GP_SCRATCH_1, GP_SCRATCH_2, JIT_ARGS, JIT_ENTRYPOINTS, JIT_RETURN_KIND, JIT_RETURN_PAYLOAD,
        JIT_RETURN_TAG, JIT_VM, SSE_SCRATCH_1, SYSV_GP_RETURN, SYSV_STACK_ALIGNMENT, rt_arg_reg,
    },
    bytecode::r#gen::BytecodeFunc,
    ir::r#gen::{
        Codegen, CodegenContext, Ctx, IRFunc, IRInstr, IRProg, IRVal, StaticConst, StaticInit,
        mktmp,
    },
    parser::ast::{BinaryExpressionKind, Literal, Type, UnaryExpressionKind},
    runtime::{
        jit::{JIT_ERROR, JIT_RET},
        rt::Rt,
        value::{Value, ValueTag},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallConv {
    SystemV,
    Jit,
}

#[derive(Debug)]
pub struct AsmProg {
    pub funcs: Vec<AsmFunc>,
    pub static_consts: Vec<StaticConst>,
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
        asm_ty: AsmType,
    },
    Ret,
    Push(AsmOp),
    Pop(AsmOp),
    Unary {
        kind: AsmInstrUnaryKind,
        dst: AsmOp,
        asm_ty: AsmType,
    },
    Shift {
        kind: AsmInstrShiftKind,
        src: AsmOp,
        dst: AsmOp,
        asm_ty: AsmType,
    },
    Bin {
        kind: AsmInstrBinKind,
        lhs: AsmOp,
        rhs: AsmOp,
        asm_ty: AsmType,
    },
    Cmp {
        src: AsmOp,
        dst: AsmOp,
        asm_ty: AsmType,
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
        conv: CallConv,
    },
    CallIndirect {
        target: AsmReg,
        conv: CallConv,
    },
    CallRt {
        target: RtFn,
    },
    Lea {
        src: AsmOp,
        dst: AsmOp,
    },
    Movsx {
        src: AsmOp,
        src_ty: AsmType,
        dst: AsmOp,
        dst_ty: AsmType,
    },
    Movzx {
        src: AsmOp,
        src_ty: AsmType,
        dst: AsmOp,
        dst_ty: AsmType,
    },
    Cvtsi2sd {
        src: AsmOp,
        dst: AsmOp,
        asm_ty: AsmType,
    },
    Cvttsd2si {
        src: AsmOp,
        dst: AsmOp,
        asm_ty: AsmType,
    },
    Cvtss2sd {
        src: AsmOp,
        dst: AsmOp,
    },
    Cvtsd2ss {
        src: AsmOp,
        dst: AsmOp,
    },
    Cvttss2si {
        src: AsmOp,
        dst: AsmOp,
        asm_ty: AsmType,
    },
    Cvtsi2ss {
        src: AsmOp,
        dst: AsmOp,
        asm_ty: AsmType,
    },
    Test {
        src: AsmOp,
        dst: AsmOp,
        asm_ty: AsmType,
    },
    Cdq {
        asm_ty: AsmType,
    },
    Div {
        op: AsmOp,
        asm_ty: AsmType,
    },
    Idiv {
        op: AsmOp,
        asm_ty: AsmType,
    },
    Comment(String),
    Newline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmType {
    Byte,
    Word,
    Long,
    Quad,
    Float,
    Double,
}

impl AsmType {
    pub fn suffix(self) -> &'static str {
        match self {
            AsmType::Byte => "b",
            AsmType::Word => "w",
            AsmType::Long => "l",
            AsmType::Quad => "q",
            AsmType::Float => "ss",
            AsmType::Double => "sd",
        }
    }

    pub fn size(&self) -> usize {
        match self {
            AsmType::Byte => 1,
            AsmType::Word => 2,
            AsmType::Long | AsmType::Float => 4,
            AsmType::Quad | AsmType::Double => 8,
        }
    }
}

impl From<Type> for AsmType {
    fn from(value: Type) -> Self {
        match value {
            Type::Int8 | Type::UInt8 | Type::Boolean => AsmType::Byte,
            Type::Int16 | Type::UInt16 => AsmType::Word,
            Type::Int32 | Type::UInt32 => AsmType::Long,
            Type::Int64 | Type::UInt64 => AsmType::Quad,
            Type::Float32 => AsmType::Float,
            Type::Float64 => AsmType::Double,
            _ => unreachable!("Not a supported type: {value:?}"),
        }
    }
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

    // Parity
    P,
    Np,
}

#[derive(Debug, Clone, Copy)]
pub enum AsmInstrUnaryKind {
    Not,
    Neg,
    Shr,
}

#[derive(Debug, Clone, Copy)]
pub enum AsmInstrShiftKind {
    Shl,
    Shr,
    Sar,
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
            ConditionCode::P => write!(f, "p"),
            ConditionCode::Np => write!(f, "np"),
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
    Mul,
    DivDouble,
    And,
    Or,
    Xor,
}

impl From<BinaryExpressionKind> for AsmInstrBinKind {
    fn from(value: BinaryExpressionKind) -> Self {
        match value {
            BinaryExpressionKind::Add => AsmInstrBinKind::Add,
            BinaryExpressionKind::Sub => AsmInstrBinKind::Sub,
            BinaryExpressionKind::Mul => AsmInstrBinKind::Mul,
            BinaryExpressionKind::BitwiseAnd => AsmInstrBinKind::And,
            BinaryExpressionKind::BitwiseOr => AsmInstrBinKind::Or,
            BinaryExpressionKind::BitwiseXor => AsmInstrBinKind::Xor,
            BinaryExpressionKind::Or => AsmInstrBinKind::Or,
            BinaryExpressionKind::And => AsmInstrBinKind::And,
            _ => unimplemented!("got {value:?}"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum AsmOp {
    Pseudo { name: String, ty: Type },
    Reg(AsmReg),
    Imm(i64),
    Mem(AsmReg, isize),
    Data(String, isize),
}

impl AsmOp {
    pub fn as_width(&self, ty: AsmType) -> String {
        match self {
            Self::Reg(reg) => reg.as_width(ty).to_string(),
            _ => self.to_string(),
        }
    }

    pub fn is_xmm(&self) -> bool {
        matches!(self, Self::Reg(reg) if reg.is_xmm())
    }
}

impl std::fmt::Display for AsmOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AsmOp::Reg(reg) => write!(f, "{}", reg.as_width(AsmType::Quad)),
            AsmOp::Imm(n) if *n < 0 => write!(f, "$-0x{:x}", n.unsigned_abs()),
            AsmOp::Imm(n) => write!(f, "$0x{n:x}"),
            AsmOp::Mem(reg, offset) if *offset < 0 => write!(
                f,
                "-0x{:x}({})",
                offset.unsigned_abs(),
                reg.as_width(AsmType::Quad)
            ),
            AsmOp::Mem(reg, offset) => write!(f, "0x{offset:x}({})", reg.as_width(AsmType::Quad)),
            AsmOp::Pseudo { .. } => unreachable!(),
            AsmOp::Data(ident, offset) => match *offset {
                0 => write!(f, "{ident}(%rip)"),
                n if n > 0 => write!(f, "{ident}+0x{n:x}(%rip)"),
                n => write!(f, "{ident}-0x{:x}(%rip)", n.unsigned_abs()),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    Xmm0,
    Xmm1,
    Xmm2,
    Xmm3,
    Xmm4,
    Xmm5,
    Xmm6,
    Xmm7,
    Xmm14,
    Xmm15,
}

impl AsmReg {
    pub fn as_width(self, ty: AsmType) -> &'static str {
        match self {
            Self::Xmm0 => "%xmm0",
            Self::Xmm1 => "%xmm1",
            Self::Xmm2 => "%xmm2",
            Self::Xmm3 => "%xmm3",
            Self::Xmm4 => "%xmm4",
            Self::Xmm5 => "%xmm5",
            Self::Xmm6 => "%xmm6",
            Self::Xmm7 => "%xmm7",
            Self::Xmm14 => "%xmm14",
            Self::Xmm15 => "%xmm15",

            _ => {
                let names = match self {
                    Self::Ax => ["%al", "%ax", "%eax", "%rax"],
                    Self::Cx => ["%cl", "%cx", "%ecx", "%rcx"],
                    Self::Dx => ["%dl", "%dx", "%edx", "%rdx"],
                    Self::Si => ["%sil", "%si", "%esi", "%rsi"],
                    Self::Di => ["%dil", "%di", "%edi", "%rdi"],
                    Self::Bp => ["%bpl", "%bp", "%ebp", "%rbp"],
                    Self::Sp => ["%spl", "%sp", "%esp", "%rsp"],
                    Self::R8 => ["%r8b", "%r8w", "%r8d", "%r8"],
                    Self::R9 => ["%r9b", "%r9w", "%r9d", "%r9"],
                    Self::R10 => ["%r10b", "%r10w", "%r10d", "%r10"],
                    Self::R11 => ["%r11b", "%r11w", "%r11d", "%r11"],
                    Self::R12 => ["%r12b", "%r12w", "%r12d", "%r12"],
                    Self::R13 => ["%r13b", "%r13w", "%r13d", "%r13"],
                    Self::R14 => ["%r14b", "%r14w", "%r14d", "%r14"],
                    Self::R15 => ["%r15b", "%r15w", "%r15d", "%r15"],
                    _ => unreachable!("got: {self:?}"),
                };

                match ty {
                    AsmType::Byte => names[0],
                    AsmType::Word => names[1],
                    AsmType::Long | AsmType::Float => names[2],
                    AsmType::Quad | AsmType::Double => names[3],
                }
            }
        }
    }

    pub fn is_xmm(self) -> bool {
        matches!(
            self,
            Self::Xmm0
                | Self::Xmm1
                | Self::Xmm2
                | Self::Xmm3
                | Self::Xmm4
                | Self::Xmm5
                | Self::Xmm6
                | Self::Xmm7
                | Self::Xmm14
                | Self::Xmm15
        )
    }
}

impl Codegen<AsmProg, CodegenContext> for IRProg {
    fn codegen(&self, ctx: &mut CodegenContext) -> Result<AsmProg, Box<dyn std::error::Error>> {
        Ok(AsmProg {
            funcs: self
                .funcs
                .iter()
                .map(|f| f.codegen(ctx))
                .collect::<Result<Vec<AsmFunc>, _>>()?,
            static_consts: ctx
                .static_consts
                .iter()
                .map(|konst| StaticConst {
                    name: konst.name.clone(),
                    init: konst.init,
                    ty: konst.ty,
                    alignment: konst.alignment,
                })
                .collect(),
        })
    }
}

impl Codegen<AsmFunc, CodegenContext> for IRFunc {
    fn codegen(&self, ctx: &mut CodegenContext) -> Result<AsmFunc, Box<dyn std::error::Error>> {
        ctx.current_fn = self.name.clone();

        let mut body = vec![];

        insert_prologue(&mut body);

        body.push(AsmInstr::Comment(format!(
            "{} = *const Value (args)",
            JIT_ARGS.as_width(AsmType::Quad)
        )));
        body.push(AsmInstr::Newline);

        for (idx, param) in self.params.iter().enumerate() {
            let offset = (idx * std::mem::size_of::<Value>() + std::mem::offset_of!(Value, payload))
                as isize;

            body.push(AsmInstr::Comment(format!(
                "Put {offset}({}) into {}",
                param.name,
                JIT_ARGS.as_width(AsmType::Quad),
            )));
            body.push(AsmInstr::Mov {
                src: AsmOp::Mem(JIT_ARGS, offset),
                dst: AsmOp::Pseudo {
                    name: param.name.clone(),
                    ty: param.ty,
                },
                asm_ty: param.ty.into(),
            });
            body.push(AsmInstr::Newline);
        }

        for instr in &self.body {
            body.extend::<Vec<AsmInstr>>(instr.codegen(ctx)?);
        }

        Ok(AsmFunc {
            name: self.name.to_owned(),
            body,
            space: 0,
        })
    }
}

impl Codegen<Vec<AsmInstr>, CodegenContext> for IRInstr {
    fn codegen(
        &self,
        ctx: &mut CodegenContext,
    ) -> Result<Vec<AsmInstr>, Box<dyn std::error::Error>> {
        match self {
            IRInstr::Cpy { src, src_ty, dst } => Ok(vec![
                AsmInstr::Comment(format!("Copying {src_ty:?}")),
                AsmInstr::Mov {
                    src: src.codegen(ctx)?,
                    dst: dst.codegen(ctx)?,
                    asm_ty: (*src_ty).into(),
                },
                AsmInstr::Newline,
            ]),

            IRInstr::Ret { val, ret_ty } => {
                let mut instrs = vec![
                    AsmInstr::Comment("Store the JIT return payload".into()),
                    AsmInstr::Mov {
                        src: val
                            .as_ref()
                            .map(|val| val.codegen(ctx))
                            .transpose()?
                            .unwrap_or(AsmOp::Imm(0)),
                        dst: AsmOp::Reg(JIT_RETURN_PAYLOAD),
                        asm_ty: ret_ty.map(|ty| ty.into()).unwrap_or(AsmType::Quad),
                    },
                    AsmInstr::Newline,
                ];

                if let Some(ty) = ret_ty {
                    let src_ty = (*ty).into();

                    match (src_ty, ty.is_signed()) {
                        (AsmType::Float | AsmType::Double, _) => {
                            /* Float32 payload bits already occupy the low 32-bits.  */
                            /* Float64 payload bits already occupy the full register.  */
                            /* No additional extension is needed.  */
                            // FIXME: add this as AsmInstr::Comment
                        }

                        /* already 64 bits  */
                        (AsmType::Quad, _) => {
                            instrs.push(AsmInstr::Comment(
                                "Already 64 bits.  No extension needed.".into(),
                            ));
                        }

                        /* writing to 32-bit register automatically zero-extends into the
                         * corresponding 64-bit register.  */
                        (AsmType::Long, false) => {
                            instrs.extend([
                                AsmInstr::Comment(
                                    "writing to 32-bit register automatically zero-extends".into(),
                                ),
                                AsmInstr::Comment("into the corresponding 64-bit register".into()),
                            ]);
                        }

                        /* sign-extend smaller ints  */
                        (_, true) => {
                            instrs.extend([
                                AsmInstr::Comment(
                                    "Extend the payload register before returning from the native fn."
                                        .into(),
                                ),
                                AsmInstr::Comment(format!(
                                    "retval ty: {ty:?}, so we need to use {}-extend it.",
                                    if ty.is_signed() { "sign" } else { "zero" }
                                )),
                            ]);
                            instrs.push(AsmInstr::Movsx {
                                src: AsmOp::Reg(JIT_RETURN_PAYLOAD),
                                src_ty,
                                dst: AsmOp::Reg(JIT_RETURN_PAYLOAD),
                                dst_ty: AsmType::Quad,
                            });
                        }

                        /* zero-extend smaller ints.  */
                        (_, false) => {
                            instrs.extend([
                                AsmInstr::Comment(
                                    "Extend the payload register before returning from the native fn."
                                        .into(),
                                ),
                                AsmInstr::Comment(format!(
                                    "retval ty: {ty:?}, so we need to use {}-extend it.",
                                    if ty.is_signed() { "sign" } else { "zero" }
                                )),
                            ]);
                            instrs.push(AsmInstr::Movzx {
                                src: AsmOp::Reg(JIT_RETURN_PAYLOAD),
                                src_ty,
                                dst: AsmOp::Reg(JIT_RETURN_PAYLOAD),
                                dst_ty: AsmType::Quad,
                            });
                        }
                    }
                }

                instrs.extend([
                    AsmInstr::Newline,
                    AsmInstr::Comment("Store the JIT return tag".into()),
                    AsmInstr::Mov {
                        src: AsmOp::Imm(ret_ty.map(ValueTag::from).unwrap_or(ValueTag::Void) as i64),
                        dst: AsmOp::Reg(JIT_RETURN_TAG),
                        asm_ty: AsmType::Quad,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Mark the JIT return kind".into()),
                    AsmInstr::Mov {
                        src: AsmOp::Imm(JIT_RET as i64),
                        dst: AsmOp::Reg(JIT_RETURN_KIND),
                        asm_ty: AsmType::Quad,
                    },
                    AsmInstr::Newline,
                ]);

                insert_epilogue(&mut instrs);
                instrs.push(AsmInstr::Ret);
                instrs.push(AsmInstr::Newline);

                Ok(instrs)
            }

            IRInstr::Bin {
                kind,
                lhs,
                rhs,
                dst,
                operand_ty,
            } if matches!(
                kind,
                BinaryExpressionKind::Add | BinaryExpressionKind::Sub | BinaryExpressionKind::Mul
            ) =>
            {
                let kind = match kind {
                    BinaryExpressionKind::Add => AsmInstrBinKind::Add,
                    BinaryExpressionKind::Sub => AsmInstrBinKind::Sub,
                    BinaryExpressionKind::Mul => AsmInstrBinKind::Mul,
                    _ => unimplemented!(),
                };

                let instrs = vec![
                    AsmInstr::Comment(format!("Binary ({kind:?})")),
                    AsmInstr::Mov {
                        src: lhs.codegen(ctx)?,
                        dst: dst.codegen(ctx)?,
                        asm_ty: (*operand_ty).into(),
                    },
                    AsmInstr::Bin {
                        kind,
                        lhs: rhs.codegen(ctx)?,
                        rhs: dst.codegen(ctx)?,
                        asm_ty: (*operand_ty).into(),
                    },
                    AsmInstr::Newline,
                ];

                Ok(instrs)
            }

            IRInstr::Bin {
                kind: BinaryExpressionKind::Div,
                lhs,
                rhs,
                dst,
                operand_ty,
            } if matches!((*operand_ty).into(), AsmType::Float | AsmType::Double) => {
                let instrs = vec![
                    AsmInstr::Comment("Floating-point division.".into()),
                    AsmInstr::Mov {
                        src: lhs.codegen(ctx)?,
                        dst: dst.codegen(ctx)?,
                        asm_ty: (*operand_ty).into(),
                    },
                    AsmInstr::Bin {
                        kind: AsmInstrBinKind::DivDouble,
                        lhs: rhs.codegen(ctx)?,
                        rhs: dst.codegen(ctx)?,
                        asm_ty: (*operand_ty).into(),
                    },
                ];

                Ok(instrs)
            }

            IRInstr::Bin {
                kind: BinaryExpressionKind::Div,
                lhs,
                rhs,
                dst,
                operand_ty,
            } if matches!((*operand_ty).into(), AsmType::Byte) && operand_ty.is_signed() => {
                let instrs = vec![
                    AsmInstr::Comment("Signed 8-bit int div".into()),
                    AsmInstr::Comment("Move the dividend into Reg::Ax".into()),
                    AsmInstr::Mov {
                        src: lhs.codegen(ctx)?,
                        dst: AsmOp::Reg(AsmReg::Ax),
                        asm_ty: AsmType::Byte,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment("For signed division, we sign-extend Reg::Ax into Reg::Dx".into()),
                    AsmInstr::Cdq { asm_ty: AsmType::Byte },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Then, we use the one-operand 'idiv' with the divisor as the operand.".into()),
                    AsmInstr::Idiv { op: rhs.codegen(ctx)?, asm_ty: AsmType::Byte },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Then, after the division, the quotient is in Reg::Ax, and the remainder is in Reg::Dx".into()),
                    AsmInstr::Mov { src: AsmOp::Reg(AsmReg::Ax), dst: dst.codegen(ctx)?, asm_ty: AsmType::Byte },
                    AsmInstr::Newline,
                ];

                Ok(instrs)
            }

            IRInstr::Bin {
                kind: BinaryExpressionKind::Div,
                lhs,
                rhs,
                dst,
                operand_ty,
            } if matches!((*operand_ty).into(), AsmType::Byte) && !operand_ty.is_signed() => {
                let instrs = vec![
                    AsmInstr::Comment("Unsigned 8-bit int div".into()),
                    AsmInstr::Comment("Move the dividend into Reg::Ax".into()),
                    AsmInstr::Mov {
                        src: lhs.codegen(ctx)?,
                        dst: AsmOp::Reg(AsmReg::Ax),
                        asm_ty: AsmType::Byte,
                    },
                    AsmInstr::Comment("For unsigned 8-bit division, we zero-extend %al to %eax, which also clear %ah".into()),
                    AsmInstr::Movzx { src: AsmOp::Reg(AsmReg::Ax), src_ty: AsmType::Byte, dst: AsmOp::Reg(AsmReg::Ax), dst_ty: AsmType::Long },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Then, we use the one-operand 'div' with the divisor as the operand.".into()),
                    AsmInstr::Div { op: rhs.codegen(ctx)?, asm_ty: AsmType::Byte },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Then, after the division, the quotient is in %al, and the remainder is in %ah".into()),
                    AsmInstr::Mov { src: AsmOp::Reg(AsmReg::Ax), dst: dst.codegen(ctx)?, asm_ty: AsmType::Byte },
                    AsmInstr::Newline,
                ];

                Ok(instrs)
            }

            IRInstr::Bin {
                kind: BinaryExpressionKind::Div,
                lhs,
                rhs,
                dst,
                operand_ty,
            } if operand_ty.is_signed() => {
                let instrs = vec![
                    AsmInstr::Comment("Signed int division (larger than Byte)".into()),
                    AsmInstr::Comment("Move the dividend into Reg::Ax".into()),
                    AsmInstr::Mov {
                        src: lhs.codegen(ctx)?,
                        dst: AsmOp::Reg(AsmReg::Ax),
                        asm_ty: (*operand_ty).into(),
                    },
                    AsmInstr::Comment("For signed division, we sign-extend Reg::Ax into Reg::Dx".into()),
                    AsmInstr::Cdq { asm_ty: (*operand_ty).into() },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Then, we use the one-operand 'idiv' with the divisor as the operand.".into()),
                    AsmInstr::Idiv { op: rhs.codegen(ctx)?, asm_ty: (*operand_ty).into() },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Then, after the division, the quotient is in Reg::Ax, and the remainder is in Reg::Dx".into()),
                    AsmInstr::Mov { src: AsmOp::Reg(AsmReg::Ax), dst: dst.codegen(ctx)?, asm_ty: (*operand_ty).into() },
                    AsmInstr::Newline,
                ];

                Ok(instrs)
            }

            IRInstr::Bin {
                kind: BinaryExpressionKind::Div,
                lhs,
                rhs,
                dst,
                operand_ty,
            } if !operand_ty.is_signed() => {
                let instrs = vec![
                    AsmInstr::Comment("Unsigned int division (larger than Byte)".into()),
                    AsmInstr::Comment("Move the dividend into Reg::Ax".into()),
                    AsmInstr::Mov {
                        src: lhs.codegen(ctx)?,
                        dst: AsmOp::Reg(AsmReg::Ax),
                        asm_ty: (*operand_ty).into(),
                    },
                    AsmInstr::Comment("For unsigned division, we zero out Reg::Dx, because".into()),
                    AsmInstr::Comment("unsigned division interprets all bits as positive values.".into()),
                    AsmInstr::Mov { src: AsmOp::Imm(0), dst: AsmOp::Reg(AsmReg::Dx), asm_ty: (*operand_ty).into() },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Then, we use the one-operand 'div' with the divisor as the operand.".into()),
                    AsmInstr::Div { op: rhs.codegen(ctx)?, asm_ty: (*operand_ty).into() },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Then, after the division, the quotient is in Reg::Ax, and the remainder is in Reg::Dx".into()),
                    AsmInstr::Mov { src: AsmOp::Reg(AsmReg::Ax), dst: dst.codegen(ctx)?, asm_ty: (*operand_ty).into() },
                    AsmInstr::Newline,
                ];

                Ok(instrs)
            }

            IRInstr::Bin {
                kind: BinaryExpressionKind::Rem,
                lhs,
                rhs,
                dst,
                operand_ty,
            } if matches!((*operand_ty).into(), AsmType::Byte) && operand_ty.is_signed() => {
                Ok(vec![
                    AsmInstr::Comment("Signed 8-bit int rem.".into()),
                    AsmInstr::Comment("Move the dividend into %al".into()),
                    AsmInstr::Mov {
                        src: lhs.codegen(ctx)?,
                        dst: AsmOp::Reg(AsmReg::Ax),
                        asm_ty: AsmType::Byte,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Sign-extend %al into %ax for signed 8-bit division.".into()),
                    AsmInstr::Movsx {
                        src: AsmOp::Reg(AsmReg::Ax),
                        src_ty: AsmType::Byte,
                        dst: AsmOp::Reg(AsmReg::Ax),
                        dst_ty: AsmType::Word,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Perform signed division using idiv.".into()),
                    AsmInstr::Idiv {
                        op: rhs.codegen(ctx)?,
                        asm_ty: AsmType::Byte,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Extract the 8-bit rem from %ah into %al.".into()),
                    AsmInstr::Shift {
                        kind: AsmInstrShiftKind::Sar,
                        src: AsmOp::Imm(8),
                        dst: AsmOp::Reg(AsmReg::Ax),
                        asm_ty: AsmType::Word,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Move the rem into the dst.".into()),
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::Ax),
                        dst: dst.codegen(ctx)?,
                        asm_ty: AsmType::Byte,
                    },
                    AsmInstr::Newline,
                ])
            }

            IRInstr::Bin {
                kind: BinaryExpressionKind::Rem,
                lhs,
                rhs,
                dst,
                operand_ty,
            } if operand_ty.is_signed() => {
                let asm_ty: AsmType = (*operand_ty).into();

                Ok(vec![
                    AsmInstr::Comment("Signed int rem (larger than Byte)".into()),
                    AsmInstr::Comment("Move the dividend into Reg::Ax".into()),
                    AsmInstr::Mov {
                        src: lhs.codegen(ctx)?,
                        dst: AsmOp::Reg(AsmReg::Ax),
                        asm_ty,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Sign-extend Reg::Ax into Reg::Dx.".into()),
                    AsmInstr::Cdq { asm_ty },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Perform signed division using idiv.".into()),
                    AsmInstr::Idiv {
                        op: rhs.codegen(ctx)?,
                        asm_ty,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Move the rem into the dst.".into()),
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::Dx),
                        dst: dst.codegen(ctx)?,
                        asm_ty,
                    },
                    AsmInstr::Newline,
                ])
            }

            IRInstr::Bin {
                kind: BinaryExpressionKind::Rem,
                lhs,
                rhs,
                dst,
                operand_ty,
            } if AsmType::from(*operand_ty) == AsmType::Byte => {
                let asm_ty: AsmType = (*operand_ty).into();

                Ok(vec![
                    AsmInstr::Comment("Unsigned 8-bit int rem.".into()),
                    AsmInstr::Comment("Move the dividend into %al".into()),
                    AsmInstr::Mov {
                        src: lhs.codegen(ctx)?,
                        dst: AsmOp::Reg(AsmReg::Ax),
                        asm_ty,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Zero-extend %al into %eax, clearing %ah.".into()),
                    AsmInstr::Movzx {
                        src: AsmOp::Reg(AsmReg::Ax),
                        src_ty: AsmType::Byte,
                        dst: AsmOp::Reg(AsmReg::Ax),
                        dst_ty: AsmType::Long,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Perform unsigned division using div.".into()),
                    AsmInstr::Div {
                        op: rhs.codegen(ctx)?,
                        asm_ty,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Extract the 8-bit rem from %ah into %al.".into()),
                    AsmInstr::Shift {
                        kind: AsmInstrShiftKind::Shr,
                        src: AsmOp::Imm(8),
                        dst: AsmOp::Reg(AsmReg::Ax),
                        asm_ty: AsmType::Word,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Move the rem into the dst.".into()),
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::Ax),
                        dst: dst.codegen(ctx)?,
                        asm_ty,
                    },
                    AsmInstr::Newline,
                ])
            }

            IRInstr::Bin {
                kind: BinaryExpressionKind::Rem,
                lhs,
                rhs,
                dst,
                operand_ty,
            } => {
                let asm_ty: AsmType = (*operand_ty).into();

                Ok(vec![
                    AsmInstr::Comment("Unsigned int rem (larger than Byte)".into()),
                    AsmInstr::Comment("Move the dividend into Reg::Ax".into()),
                    AsmInstr::Mov {
                        src: lhs.codegen(ctx)?,
                        dst: AsmOp::Reg(AsmReg::Ax),
                        asm_ty,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Clear Reg::Dx for unsigned division.".into()),
                    AsmInstr::Mov {
                        src: AsmOp::Imm(0),
                        dst: AsmOp::Reg(AsmReg::Dx),
                        asm_ty,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Perform unsigned division using div.".into()),
                    AsmInstr::Div {
                        op: rhs.codegen(ctx)?,
                        asm_ty,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Move the rem into the dst.".into()),
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::Dx),
                        dst: dst.codegen(ctx)?,
                        asm_ty,
                    },
                    AsmInstr::Newline,
                ])
            }

            IRInstr::Bin {
                kind,
                lhs,
                rhs,
                dst,
                operand_ty,
            } if matches!(
                kind,
                BinaryExpressionKind::BitwiseAnd
                    | BinaryExpressionKind::BitwiseOr
                    | BinaryExpressionKind::BitwiseXor
            ) =>
            {
                let instrs = vec![
                    AsmInstr::Mov {
                        src: lhs.codegen(ctx)?,
                        dst: dst.codegen(ctx)?,
                        asm_ty: (*operand_ty).into(),
                    },
                    AsmInstr::Bin {
                        kind: (*kind).into(),
                        lhs: rhs.codegen(ctx)?,
                        rhs: dst.codegen(ctx)?,
                        asm_ty: (*operand_ty).into(),
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
            } if matches!(kind, BinaryExpressionKind::Shl | BinaryExpressionKind::Shr) => {
                let asm_ty: AsmType = (*operand_ty).into();
                let is_signed = operand_ty.is_signed();

                let shift_kind = match (*kind, is_signed) {
                    (BinaryExpressionKind::Shl, _) => AsmInstrShiftKind::Shl,
                    (BinaryExpressionKind::Shr, true) => AsmInstrShiftKind::Sar,
                    (BinaryExpressionKind::Shr, false) => AsmInstrShiftKind::Shr,
                    _ => unreachable!(),
                };

                /* Rust's wrapping_shl/wrapping_shr use the shift count
                 * modulo the operand's bit width.
                 *
                 * i8/u8:   count & 7
                 * i16/u16: count & 15
                 * i32/u32: count & 31
                 * i64/u64: count & 63  */
                let shift_mask = (operand_ty.size_of() * 8 - 1) as i64;

                let instrs = vec![
                    AsmInstr::Comment(format!(
                        "Bitwise shift ({shift_kind:?}, signed: {is_signed})"
                    )),
                    AsmInstr::Newline,
                    AsmInstr::Comment("Move the shift count into Reg::Cx.".into()),
                    AsmInstr::Mov {
                        src: rhs.codegen(ctx)?,
                        dst: AsmOp::Reg(AsmReg::Cx),
                        asm_ty,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment(format!("Mask CL with {shift_mask}.")),
                    AsmInstr::Bin {
                        kind: AsmInstrBinKind::And,
                        lhs: AsmOp::Imm(shift_mask),
                        rhs: AsmOp::Reg(AsmReg::Cx),
                        asm_ty: AsmType::Byte,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Move the left operand into tmp scratch #2.".into()),
                    AsmInstr::Mov {
                        src: lhs.codegen(ctx)?,
                        dst: AsmOp::Reg(GP_SCRATCH_2),
                        asm_ty,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Perform the shift using CL.".into()),
                    AsmInstr::Shift {
                        kind: shift_kind,
                        src: AsmOp::Reg(AsmReg::Cx),
                        dst: AsmOp::Reg(GP_SCRATCH_2),
                        asm_ty,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Move the shifted result into the destination.".into()),
                    AsmInstr::Mov {
                        src: AsmOp::Reg(GP_SCRATCH_2),
                        dst: dst.codegen(ctx)?,
                        asm_ty,
                    },
                    AsmInstr::Newline,
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
                if operand_ty.is_float() {
                    let cc = match kind {
                        BinaryExpressionKind::Less => ConditionCode::B,
                        BinaryExpressionKind::Greater => ConditionCode::A,
                        BinaryExpressionKind::LessEqual => ConditionCode::Be,
                        BinaryExpressionKind::GreaterEqual => ConditionCode::Ae,
                        BinaryExpressionKind::Equal => ConditionCode::E,
                        BinaryExpressionKind::NotEqual => ConditionCode::Ne,
                        _ => unreachable!("got: {kind:?}"),
                    };

                    let mut instrs = vec![
                        AsmInstr::Comment("Compare the two floating-point values.".into()),
                        AsmInstr::Comment("The processor sets CF, ZF, and PF.".into()),
                        AsmInstr::Cmp {
                            src: rhs.codegen(ctx)?,
                            dst: lhs.codegen(ctx)?,
                            asm_ty: (*operand_ty).into(),
                        },
                        AsmInstr::Newline,
                        AsmInstr::Comment(
                            "Update byte in dst if one of the conditions is true.".into(),
                        ),
                        AsmInstr::Setcc {
                            cc,
                            dst: dst.codegen(ctx)?,
                        },
                    ];

                    if *kind == BinaryExpressionKind::NotEqual {
                        instrs.extend([
                            AsmInstr::Comment(format!(
                                "For {kind:?}, we also need to check the P flag."
                            )),
                            AsmInstr::Setcc {
                                cc: ConditionCode::P,
                                dst: AsmOp::Reg(GP_SCRATCH_2),
                            },
                            AsmInstr::Newline,
                            AsmInstr::Comment("Combine the result using the OR semantics.".into()),
                            AsmInstr::Bin {
                                kind: AsmInstrBinKind::Or,
                                lhs: AsmOp::Reg(GP_SCRATCH_2),
                                rhs: dst.codegen(ctx)?,
                                asm_ty: AsmType::Byte,
                            },
                            AsmInstr::Newline,
                        ]);
                    } else {
                        instrs.extend([
                            AsmInstr::Comment(format!(
                                "For {kind:?}, we also need to check the NP flag."
                            )),
                            AsmInstr::Setcc {
                                cc: ConditionCode::Np,
                                dst: AsmOp::Reg(GP_SCRATCH_2),
                            },
                            AsmInstr::Newline,
                            AsmInstr::Comment("Combine the result using the And semantics.".into()),
                            AsmInstr::Bin {
                                kind: AsmInstrBinKind::And,
                                lhs: AsmOp::Reg(GP_SCRATCH_2),
                                rhs: dst.codegen(ctx)?,
                                asm_ty: AsmType::Byte,
                            },
                            AsmInstr::Newline,
                        ]);
                    }

                    Ok(instrs)
                } else {
                    let cc = match (*kind, !operand_ty.is_signed()) {
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
                        _ => unreachable!("got: {kind:?} {}", !operand_ty.is_signed()),
                    };

                    let instrs = vec![
                        AsmInstr::Comment(format!("Comparison op {kind:?}")),
                        AsmInstr::Comment(
                            "'cmp' computes `dst - src`, and updates the processor's status flags"
                                .into(),
                        ),
                        AsmInstr::Comment("(ZF, SF, OF)".into()),
                        AsmInstr::Cmp {
                            src: rhs.codegen(ctx)?,
                            dst: lhs.codegen(ctx)?,
                            asm_ty: (*operand_ty).into(),
                        },
                        AsmInstr::Newline,
                        AsmInstr::Comment(format!(
                            "Then, `{}` writes `1` if the previous comparison satisfied the {kind:?} condition.",
                            format!("set{cc:?}").to_lowercase()
                        )),
                        AsmInstr::Setcc {
                            cc,
                            dst: dst.codegen(ctx)?,
                        },
                        AsmInstr::Newline,
                    ];

                    Ok(instrs)
                }
            }

            IRInstr::Unary {
                kind,
                src,
                dst,
                operand_ty,
            } => {
                let mut instrs = vec![
                    AsmInstr::Newline,
                    AsmInstr::Comment(format!("Unary {kind:?}")),
                ];

                let src = src.codegen(ctx)?;
                let dst = dst.codegen(ctx)?;

                instrs.extend([AsmInstr::Mov {
                    src,
                    dst: dst.clone(),
                    asm_ty: (*operand_ty).into(),
                }]);

                match kind {
                    UnaryExpressionKind::Negate if operand_ty.is_float() => {
                        let float_ty: AsmType = (*operand_ty).into();

                        instrs.push(AsmInstr::Comment(
                            "Move the floating-point bits into tmp scratch.".into(),
                        ));
                        instrs.push(AsmInstr::Mov {
                            src: dst.clone(),
                            dst: AsmOp::Reg(GP_SCRATCH_1),
                            asm_ty: float_ty,
                        });
                        instrs.push(AsmInstr::Newline);

                        match float_ty {
                            AsmType::Float => {
                                instrs.push(AsmInstr::Comment(
                                    "Flip bit 31 of the f32 representation.".into(),
                                ));
                                instrs.push(AsmInstr::Bin {
                                    kind: AsmInstrBinKind::Xor,
                                    lhs: AsmOp::Imm(0x8000_0000),
                                    rhs: AsmOp::Reg(GP_SCRATCH_1),
                                    asm_ty: AsmType::Long,
                                });
                                instrs.push(AsmInstr::Newline);
                            }

                            AsmType::Double => {
                                instrs.push(AsmInstr::Comment(
                                    "Construct the f64 sign-bit mask in tmp scratch #2.".into(),
                                ));
                                instrs.push(AsmInstr::Mov {
                                    src: AsmOp::Imm(1),
                                    dst: AsmOp::Reg(GP_SCRATCH_2),
                                    asm_ty: AsmType::Quad,
                                });
                                instrs.push(AsmInstr::Shift {
                                    kind: AsmInstrShiftKind::Shl,
                                    src: AsmOp::Imm(63),
                                    dst: AsmOp::Reg(GP_SCRATCH_2),
                                    asm_ty: AsmType::Quad,
                                });
                                instrs.push(AsmInstr::Newline);
                                instrs.push(AsmInstr::Comment(
                                    "Flip bit 63 of the f64 representation.".into(),
                                ));
                                instrs.push(AsmInstr::Bin {
                                    kind: AsmInstrBinKind::Xor,
                                    lhs: AsmOp::Reg(GP_SCRATCH_2),
                                    rhs: AsmOp::Reg(GP_SCRATCH_1),
                                    asm_ty: AsmType::Quad,
                                });
                                instrs.push(AsmInstr::Newline);
                            }

                            _ => unreachable!(),
                        }

                        instrs.push(AsmInstr::Comment(
                            "Move the negated bits back into the dst.".into(),
                        ));
                        instrs.push(AsmInstr::Mov {
                            src: AsmOp::Reg(GP_SCRATCH_1),
                            dst: dst.clone(),
                            asm_ty: float_ty,
                        });
                        instrs.push(AsmInstr::Newline);
                    }
                    UnaryExpressionKind::Negate if operand_ty.is_signed() => {
                        instrs.push(AsmInstr::Unary {
                            kind: AsmInstrUnaryKind::Neg,
                            dst: dst.clone(),
                            asm_ty: (*operand_ty).into(),
                        });
                        instrs.push(AsmInstr::Newline);
                    }
                    UnaryExpressionKind::LogicalNot if *operand_ty == Type::Boolean => {
                        instrs.extend([
                            AsmInstr::Newline,
                            AsmInstr::Comment("Compare the operand against 0.".into()),
                            AsmInstr::Comment("If dst == 0, the Zero Flag (ZF) is set.".into()),
                            AsmInstr::Cmp {
                                src: AsmOp::Imm(0),
                                dst: dst.clone(),
                                asm_ty: (*operand_ty).into(),
                            },
                            AsmInstr::Newline,
                            AsmInstr::Comment(
                                "Clear the dst before sete, which only writes the lowest 8 bits,"
                                    .into(),
                            ),
                            AsmInstr::Comment(
                                "meaning we must ensure the upper 56 bits are zero.".into(),
                            ),
                            AsmInstr::Mov {
                                src: AsmOp::Imm(0),
                                dst: dst.clone(),
                                asm_ty: (*operand_ty).into(),
                            },
                            AsmInstr::Newline,
                            AsmInstr::Comment(
                                "If ZF is set (dst was zero), set the lowest byte to 1, else 0."
                                    .into(),
                            ),
                            AsmInstr::Setcc {
                                cc: ConditionCode::E,
                                dst,
                            },
                        ]);
                    }
                    UnaryExpressionKind::BitwiseNot if operand_ty.is_numeric() => {
                        instrs.push(AsmInstr::Unary {
                            kind: AsmInstrUnaryKind::Not,
                            dst: dst.clone(),
                            asm_ty: (*operand_ty).into(),
                        });
                        instrs.push(AsmInstr::Newline);
                    }
                    _ => panic!("Unsupported unary op {kind:?} on {operand_ty:?}"), // FIXME: don't panic
                }

                instrs.push(AsmInstr::Newline);

                Ok(instrs)
            }

            IRInstr::Print { val, ty } => {
                let val = val.codegen(ctx)?;
                let tag = ValueTag::from(*ty) as u64;

                let vm_arg = rt_arg_reg(RtFn::Print, 0);
                let tag_arg = rt_arg_reg(RtFn::Print, 1);
                let payload_arg = rt_arg_reg(RtFn::Print, 2);

                let body = vec![
                    AsmInstr::Comment(format!("{} = *mut Vm", vm_arg.as_width(AsmType::Quad))),
                    AsmInstr::Mov {
                        src: AsmOp::Reg(JIT_VM),
                        dst: AsmOp::Reg(vm_arg),
                        asm_ty: AsmType::Quad,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment(format!("{} = payload", payload_arg.as_width(AsmType::Quad))),
                    match (ty.size_of(), ty.is_signed()) {
                        (4, _) if *ty == Type::Float32 => AsmInstr::Mov {
                            src: val,
                            dst: AsmOp::Reg(payload_arg),
                            asm_ty: AsmType::Float,
                        },
                        (1 | 2 | 4, false) => AsmInstr::Movzx {
                            src: val,
                            src_ty: (*ty).into(),
                            dst: AsmOp::Reg(payload_arg),
                            dst_ty: AsmType::Quad,
                        },
                        (1 | 2 | 4, true) => AsmInstr::Movsx {
                            src: val,
                            src_ty: (*ty).into(),
                            dst: AsmOp::Reg(payload_arg),
                            dst_ty: AsmType::Quad,
                        },
                        _ => AsmInstr::Mov {
                            src: val,
                            dst: AsmOp::Reg(payload_arg),
                            asm_ty: AsmType::Quad,
                        },
                    },
                    AsmInstr::Comment(format!("{} = ValueTag", tag_arg.as_width(AsmType::Quad))),
                    AsmInstr::Mov {
                        src: AsmOp::Imm(tag as i64),
                        dst: AsmOp::Reg(tag_arg),
                        asm_ty: AsmType::Quad,
                    },
                    AsmInstr::Newline,
                    AsmInstr::CallRt {
                        target: RtFn::Print,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Test {
                        src: AsmOp::Reg(SYSV_GP_RETURN),
                        dst: AsmOp::Reg(SYSV_GP_RETURN),
                        asm_ty: AsmType::Quad,
                    },
                    AsmInstr::Jmpcc {
                        cc: ConditionCode::Ne,
                        target: format!("{}_rt_err", ctx.current_fn),
                    },
                    AsmInstr::Newline,
                ];

                Ok(body)
            }

            IRInstr::Jz { cond, target } => Ok(vec![
                AsmInstr::Comment("`jz`".into()),
                AsmInstr::Comment("Compare condition against 0.".into()),
                AsmInstr::Cmp {
                    src: AsmOp::Imm(0),
                    dst: cond.codegen(ctx)?,
                    asm_ty: AsmType::Byte,
                },
                AsmInstr::Newline,
                AsmInstr::Comment(
                    "Jump to target label if the previous instr compared true.".into(),
                ),
                AsmInstr::Comment("Otherwise, fall through below.".into()),
                AsmInstr::Jmpcc {
                    cc: ConditionCode::E,
                    target: format!(".L{}_{}", ctx.current_fn, target),
                },
                AsmInstr::Newline,
            ]),

            IRInstr::Jmp { target } => Ok(vec![AsmInstr::Jmp {
                target: format!(".L{}_{}", ctx.current_fn, target),
            }]),

            IRInstr::Label(target) => Ok(vec![AsmInstr::Label(format!(
                ".L{}_{}",
                ctx.current_fn, target
            ))]),

            IRInstr::Call {
                target,
                args,
                arg_tys,
                dst,
            } => {
                let func = ctx.get::<BytecodeFunc>(target).unwrap();

                let vm_arg = rt_arg_reg(RtFn::Call, 0);
                let function_arg = rt_arg_reg(RtFn::Call, 1);
                let args_arg = rt_arg_reg(RtFn::Call, 2);
                let count_arg = rt_arg_reg(RtFn::Call, 3);
                let result_arg = rt_arg_reg(RtFn::Call, 4);

                let argc = args.len();
                let args_space = argc * std::mem::size_of::<Value>();
                let args_offset = std::mem::size_of::<Value>();

                let retval_offset = 0;
                let retval_space = std::mem::size_of::<Value>();

                let frame_space = (args_space + retval_space).div_ceil(SYSV_STACK_ALIGNMENT)
                    * SYSV_STACK_ALIGNMENT;

                let mut body = vec![
                    AsmInstr::Comment(format!("Call `{target}`.")),
                    AsmInstr::Comment(format!(
                        "Reserve {frame_space} bytes for call args ({argc} args) and retval."
                    )),
                    AsmInstr::Comment("Layout:".into()),
                    AsmInstr::Comment(format!("- {args_space} bytes of native Value arguments,")),
                    AsmInstr::Comment(format!(
                        "- {retval_space} bytes for the runtime return Value,",
                    )),
                    AsmInstr::Comment("All this space is rounded up to 16-byte alignment".into()),
                    AsmInstr::Bin {
                        kind: AsmInstrBinKind::Sub,
                        lhs: AsmOp::Imm(frame_space as i64),
                        rhs: AsmOp::Reg(AsmReg::Sp),
                        asm_ty: AsmType::Quad,
                    },
                    AsmInstr::Newline,
                ];

                body.extend([AsmInstr::Comment("Prepare Value args.".into())]);

                for (idx, arg) in args.iter().enumerate() {
                    let base = args_offset + idx * std::mem::size_of::<Value>();
                    let tag = ValueTag::from(arg_tys[idx]);

                    body.extend([
                        AsmInstr::Comment(format!(
                            "Construct Value for arg {idx}: tag={tag:?}, payload follows."
                        )),
                        AsmInstr::Comment(
                            "Store the tag first as fixup pass might clobber it.".into(),
                        ),
                        AsmInstr::Mov {
                            src: AsmOp::Imm(tag as i64),
                            dst: AsmOp::Mem(
                                AsmReg::Sp,
                                (base + std::mem::offset_of!(Value, tag)) as isize,
                            ),
                            asm_ty: AsmType::Quad,
                        },
                        AsmInstr::Newline,
                        AsmInstr::Comment(format!(
                            "Put arg {idx} into tmp scratch ({}).",
                            GP_SCRATCH_1.as_width(AsmType::Quad)
                        )),
                        AsmInstr::Mov {
                            src: arg.codegen(ctx)?,
                            dst: AsmOp::Reg(GP_SCRATCH_1),
                            asm_ty: arg_tys[idx].into(),
                        },
                        AsmInstr::Newline,
                        AsmInstr::Comment(
                            "Store the scratch value into the payload field.".to_string(),
                        ),
                        AsmInstr::Mov {
                            src: AsmOp::Reg(GP_SCRATCH_1),
                            dst: AsmOp::Mem(
                                AsmReg::Sp,
                                (base + std::mem::offset_of!(Value, payload)) as isize,
                            ),
                            asm_ty: AsmType::Quad,
                        },
                        AsmInstr::Newline,
                    ]);
                }

                let id = mktmp();

                let fallback_label = format!(".L{}_call_fallback_{id}", ctx.current_fn);
                let done_label = format!(".L{}_call_done_{id}", ctx.current_fn);

                body.extend([
                    AsmInstr::Comment(format!(
                        "Determine whether `{target}` has already been jitted." 
                    )),
                    AsmInstr::Comment(format!(
                        "{} = `native_entrypoints`", JIT_ENTRYPOINTS.as_width(AsmType::Quad),
                    )),
                    AsmInstr::Comment(format!(
                        "Load `native_entrypoints[{func}]` from offset {} into {}.",
                        func * std::mem::size_of::<usize>(),
                        GP_SCRATCH_1.as_width(AsmType::Quad),
                    )),
                    AsmInstr::Comment(
                        "A zero entry means the function has not been jitted.".into(),
                    ),
                    AsmInstr::Comment(
                        "A nonzero entry is its code address.".into()
                    ),
                    AsmInstr::Mov {
                        src: AsmOp::Mem(
                            JIT_ENTRYPOINTS,
                            (func * std::mem::size_of::<usize>()) as isize,
                        ),
                        dst: AsmOp::Reg(GP_SCRATCH_1),
                        asm_ty: AsmType::Quad,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment(
                        "Compare the entrypoint against zero.".into(),
                    ),
                    AsmInstr::Comment(
                        "cmp updates the cpu flags with no changes to the call target.".into(),
                    ),
                    AsmInstr::Comment(
                        "If tmp scratch == 0, the Zero Flag (ZF) is set.".into()
                    ),
                    AsmInstr::Cmp {
                        src: AsmOp::Imm(0),
                        dst: AsmOp::Reg(GP_SCRATCH_1),
                        asm_ty: AsmType::Quad,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment(
                        "If ZF is set, jump to the Vm fallback and exec the uncompiled function.".into()
                    ),
                    AsmInstr::Comment(
                        "Otherwise, fall through to the native call path, with the scratch register holding the fn addr.".into()
                    ),
                    AsmInstr::Jmpcc {
                        cc: ConditionCode::E,
                        target: fallback_label.clone(),
                    },
                    AsmInstr::Newline,
                ]);

                body.extend([
                    AsmInstr::Comment(
                        "The internal ABI expects its first argument to point to an array of Value objects."
                            .into(),
                    ),
                    AsmInstr::Comment(format!(
                        "This array has already been constructed on the stack, starting at {retval_space}(%rsp)."
                    )),
                    AsmInstr::Comment(format!("Compute its address and pass that pointer in {}.", JIT_ARGS.as_width(AsmType::Quad))),
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::Sp),
                        dst: AsmOp::Reg(JIT_ARGS),
                        asm_ty: AsmType::Quad,
                    },
                    AsmInstr::Bin {
                        kind: AsmInstrBinKind::Add,
                        lhs: AsmOp::Imm(retval_space as i64),
                        rhs: AsmOp::Reg(JIT_ARGS),
                        asm_ty: AsmType::Quad,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment(format!(
                        "Call the native implementation of `{target}` directly."
                    )),
                    AsmInstr::Comment(
                        "The scratch register holds the entrypoint address loaded from `native_entrypoints`.".into(),
                    ),
                    AsmInstr::Comment(format!(
                        "The callee receives its arguments through the JIT argument register and inherits the",

                    )),
                    AsmInstr::Comment(
                        "JIT context in (Vm, Rt, `native entrypoints`)."
                            .into(),
                    ),
                    AsmInstr::Comment(
                        "On return: the JIT payload, tag and exit-kind registers carry the result."
                            .into(),
                    ),
                    AsmInstr::CallIndirect { target: GP_SCRATCH_1, conv: CallConv::Jit },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Check whether the callee returned JIT_ERROR.".into()),
                    AsmInstr::Comment(
                        "cmp sets ZF if the exit kind is JIT_ERROR; je then branches to our error handler."
                            .into(),
                    ),
                    AsmInstr::Comment(
                        "On error, the payload register still contains the error code.  The handler propagates".into(),
                    ),
                    AsmInstr::Comment(
                        "it to our caller and restores the stack frame."
                            .into(),
                    ),
                    AsmInstr::Cmp {
                        src: AsmOp::Imm(JIT_ERROR as i64),
                        dst: AsmOp::Reg(JIT_RETURN_KIND),
                        asm_ty: AsmType::Quad,
                    },
                    AsmInstr::Jmpcc {
                        cc: ConditionCode::E,
                        target: format!("{}_rt_err", ctx.current_fn),
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment(
                        "The native call succeeded.  The return payload is in the JIT payload register."
                            .into(),
                    ),
                    AsmInstr::Comment(
                        "The shared continuation expects the result in the scratch register and a runtime".into(),
                    ),
                    AsmInstr::Comment(
                        "status code in the System V return register, as `jit_call()` does."
                            .into(),
                    ),
                    AsmInstr::Comment(
                        "Move the payload into tmp scratch before setting the runtime status 0 (success)."
                            .into(),
                    ),
                    AsmInstr::Mov {
                        src: AsmOp::Reg(JIT_RETURN_PAYLOAD),
                        dst: AsmOp::Reg(GP_SCRATCH_1),
                        asm_ty: AsmType::Quad,
                    },
                    AsmInstr::Mov {
                        src: AsmOp::Imm(0),
                        dst: AsmOp::Reg(SYSV_GP_RETURN),
                        asm_ty: AsmType::Quad,
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
                    AsmInstr::Comment(format!("{} = *mut Vm", vm_arg.as_width(AsmType::Quad))),
                    AsmInstr::Mov {
                        src: AsmOp::Reg(JIT_VM),
                        dst: AsmOp::Reg(vm_arg),
                        asm_ty: AsmType::Quad,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment(format!("{} = fn idx", function_arg.as_width(AsmType::Quad))),
                    AsmInstr::Mov {
                        src: AsmOp::Imm(func as i64),
                        dst: AsmOp::Reg(function_arg),
                        asm_ty: AsmType::Quad,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment(format!(
                        "{} = *const Value (ptr to args)",
                        args_arg.as_width(AsmType::Quad)
                    )),
                    AsmInstr::Lea {
                        src: AsmOp::Mem(AsmReg::Sp, args_offset as isize),
                        dst: AsmOp::Reg(args_arg),
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment(format!("{} = argc", count_arg.as_width(AsmType::Quad))),
                    AsmInstr::Mov {
                        src: AsmOp::Imm(argc as i64),
                        dst: AsmOp::Reg(count_arg),
                        asm_ty: AsmType::Quad,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment(format!(
                        "{} = ptr to retval",
                        result_arg.as_width(AsmType::Quad)
                    )),
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::Sp),
                        dst: AsmOp::Reg(result_arg),
                        asm_ty: AsmType::Quad,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Invoke the rt trampoline".into()),
                    AsmInstr::CallRt { target: RtFn::Call },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Read in Value.payload into tmp scratch.".into()),
                    AsmInstr::Mov {
                        src: AsmOp::Mem(
                            AsmReg::Sp,
                            (retval_offset + std::mem::offset_of!(Value, payload)) as isize,
                        ),
                        dst: AsmOp::Reg(GP_SCRATCH_1),
                        asm_ty: AsmType::Quad,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Both native and fallback calls keep on going here.".into()),
                    AsmInstr::Label(done_label),
                    AsmInstr::Comment("Release tmp space.".into()),
                    AsmInstr::Bin {
                        kind: AsmInstrBinKind::Add,
                        lhs: AsmOp::Imm(frame_space as i64),
                        rhs: AsmOp::Reg(AsmReg::Sp),
                        asm_ty: AsmType::Quad,
                    },
                    AsmInstr::Newline,
                    AsmInstr::Comment("Check status.".into()),
                    AsmInstr::Test {
                        src: AsmOp::Reg(SYSV_GP_RETURN),
                        dst: AsmOp::Reg(SYSV_GP_RETURN),
                        asm_ty: AsmType::Quad,
                    },
                    AsmInstr::Jmpcc {
                        cc: ConditionCode::Ne,
                        target: format!("{}_rt_err", ctx.current_fn),
                    },
                    AsmInstr::Newline,
                ]);

                let result_ty = match dst {
                    IRVal::Var { ty, .. } => *ty,
                    _ => unreachable!(),
                };

                body.extend([
                    AsmInstr::Comment("Store the result from tmp scratch into IR dst.".into()),
                    AsmInstr::Mov {
                        src: AsmOp::Reg(GP_SCRATCH_1),
                        dst: dst.codegen(ctx)?,
                        asm_ty: result_ty.into(),
                    },
                    AsmInstr::Newline,
                ]);

                Ok(body)
            }

            IRInstr::Truncate {
                src,
                dst,
                src_ty,
                dst_ty,
            } => Ok(vec![
                AsmInstr::Comment(format!("Truncating {src_ty:?} into {dst_ty:?}")),
                AsmInstr::Mov {
                    src: src.codegen(ctx)?,
                    dst: dst.codegen(ctx)?,
                    asm_ty: (*dst_ty).into(),
                },
                AsmInstr::Newline,
            ]),

            IRInstr::ZeroExtend {
                src,
                dst,
                src_ty,
                dst_ty,
            } => Ok(vec![
                AsmInstr::Comment(format!("Zero-extending {src_ty:?} into {dst_ty:?}")),
                AsmInstr::Movzx {
                    src: src.codegen(ctx)?,
                    src_ty: (*src_ty).into(),
                    dst: dst.codegen(ctx)?,
                    dst_ty: (*dst_ty).into(),
                },
                AsmInstr::Newline,
            ]),

            IRInstr::SignExtend {
                src,
                src_ty,
                dst,
                dst_ty,
            } => Ok(vec![
                AsmInstr::Comment(format!("Sign-extending {src_ty:?} into {dst_ty:?}")),
                AsmInstr::Movsx {
                    src: src.codegen(ctx)?,
                    src_ty: (*src_ty).into(),
                    dst: dst.codegen(ctx)?,
                    dst_ty: (*dst_ty).into(),
                },
                AsmInstr::Newline,
            ]),

            IRInstr::UIntToDouble {
                src,
                src_ty,
                dst,
                dst_ty: _,
            } => {
                let asm_ty = (*src_ty).into();
                match asm_ty {
                    AsmType::Byte | AsmType::Word => Ok(vec![
                        AsmInstr::Movzx {
                            src: src.codegen(ctx)?,
                            src_ty: asm_ty,
                            dst: AsmOp::Reg(AsmReg::Ax),
                            dst_ty: AsmType::Long,
                        },
                        AsmInstr::Cvtsi2sd {
                            src: AsmOp::Reg(AsmReg::Ax),
                            dst: dst.codegen(ctx)?,
                            asm_ty: AsmType::Long,
                        },
                    ]),
                    AsmType::Long => Ok(vec![
                        AsmInstr::Mov {
                            src: src.codegen(ctx)?,
                            dst: AsmOp::Reg(AsmReg::Ax),
                            asm_ty: AsmType::Long,
                        },
                        AsmInstr::Cvtsi2sd {
                            src: AsmOp::Reg(AsmReg::Ax),
                            dst: dst.codegen(ctx)?,
                            asm_ty: AsmType::Quad,
                        },
                    ]),
                    AsmType::Quad => {
                        let tmp = mktmp();

                        let l1 = format!(".L{}_UintToDoubleQuad1_{tmp}", ctx.current_fn);
                        let l2 = format!(".L{}_UintToDoubleQuad2_{tmp}", ctx.current_fn);

                        Ok(vec![
                            AsmInstr::Comment("Convert UInt64 to Float64.".into()),        
                            AsmInstr::Comment("cvtsi2sd interprets its source as signed, so values >= 2^63 need special handling.".into()),
                            AsmInstr::Newline,
                            AsmInstr::Comment("Compare the source against zero using signed comparison.".into()),
                            AsmInstr::Cmp {
                                src: AsmOp::Imm(0),
                                dst: src.codegen(ctx)?,
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Newline,
                            AsmInstr::Comment("If the sign bit is set, jump to the unsigned conversion path.".into()),
                            AsmInstr::Jmpcc {
                                cc: ConditionCode::L,
                                target: l1.clone(),
                            },
                            AsmInstr::Newline,
                            AsmInstr::Comment("The value fits into a signed 64-bit integer.".into()),
                            AsmInstr::Comment("Convert it directly to double and skip the unsigned conversion path.".into()),
                            AsmInstr::Cvtsi2sd {
                                src: src.codegen(ctx)?,
                                dst: dst.codegen(ctx)?,
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Jmp { target: l2.clone() },
                            AsmInstr::Newline,
                            AsmInstr::Label(l1.clone()),
                            AsmInstr::Comment("The source is >= 2^63 and cannot be converted directly as a signed integer.".into()),
                            AsmInstr::Comment("Compute (src >> 1) | (src & 1), convert to double, then multiply by 2.".into()),
                            AsmInstr::Newline,
                            AsmInstr::Comment("Load the original value into %rax.".into()),
                            AsmInstr::Mov {
                                src: src.codegen(ctx)?,
                                dst: AsmOp::Reg(AsmReg::Ax),
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Newline,
                            AsmInstr::Comment("Copy the original value into %rdx.".into()),
                            AsmInstr::Mov {
                                src: AsmOp::Reg(AsmReg::Ax),
                                dst: AsmOp::Reg(AsmReg::Dx),
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Newline,
                            AsmInstr::Comment("Logically shift %rdx right by one bit.".into()),
                            AsmInstr::Comment("This clears the sign bit and divides the unsigned value by 2.".into()),
                            AsmInstr::Unary {
                                kind: AsmInstrUnaryKind::Shr,
                                dst: AsmOp::Reg(AsmReg::Dx),
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Newline,
                            AsmInstr::Comment("Extract the original least significant bit into %rax.".into()),
                            AsmInstr::Comment("This bit preserves rounding information lost by the right shift.".into()),
                            AsmInstr::Bin {
                                kind: AsmInstrBinKind::And,
                                lhs: AsmOp::Imm(1),
                                rhs: AsmOp::Reg(AsmReg::Ax),
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Newline,
                            AsmInstr::Comment("Combine the shifted value with the original least significant bit.".into()),
                            AsmInstr::Comment("%rdx = (src >> 1) | (src & 1).".into()),
                            AsmInstr::Bin {
                                kind: AsmInstrBinKind::Or,
                                lhs: AsmOp::Reg(AsmReg::Ax),
                                rhs: AsmOp::Reg(AsmReg::Dx),
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Newline,
                            AsmInstr::Comment("The adjusted value now fits into a signed 64-bit integer.".into()),
                            AsmInstr::Comment("Convert the adjusted value to double.".into()),
                            AsmInstr::Cvtsi2sd {
                                src: AsmOp::Reg(AsmReg::Dx),
                                dst: dst.codegen(ctx)?,
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Newline,
                            AsmInstr::Comment("Double the converted result to restore the original magnitude.".into()),
                            AsmInstr::Comment("Adding the double to itself is equivalent to multiplying by 2.".into()),
                            AsmInstr::Bin {
                                kind: AsmInstrBinKind::Add,
                                lhs: dst.codegen(ctx)?,
                                rhs: dst.codegen(ctx)?,
                                asm_ty: AsmType::Double,
                            },
                            AsmInstr::Label(l2.clone()),
                        ])
                    }
                    _ => unreachable!(),
                }
            }

            IRInstr::DoubleToInt {
                src,
                dst,
                src_ty: _,
                dst_ty,
            } => {
                let asm_ty = (*dst_ty).into();
                match asm_ty {
                    AsmType::Byte | AsmType::Word => Ok(vec![
                        AsmInstr::Cvttsd2si {
                            src: src.codegen(ctx)?,
                            dst: AsmOp::Reg(AsmReg::Ax),
                            asm_ty: AsmType::Long,
                        },
                        AsmInstr::Mov {
                            src: AsmOp::Reg(AsmReg::Ax),
                            dst: dst.codegen(ctx)?,
                            asm_ty,
                        },
                    ]),
                    AsmType::Long | AsmType::Quad => Ok(vec![AsmInstr::Cvttsd2si {
                        asm_ty,
                        src: src.codegen(ctx)?,
                        dst: dst.codegen(ctx)?,
                    }]),
                    _ => unreachable!(),
                }
            }

            IRInstr::DoubleToUInt {
                src,
                src_ty: _,
                dst,
                dst_ty,
            } => {
                let asm_ty = (*dst_ty).into();

                match asm_ty {
                    AsmType::Byte | AsmType::Word => Ok(vec![
                        AsmInstr::Cvttsd2si {
                            src: src.codegen(ctx)?,
                            dst: AsmOp::Reg(AsmReg::Ax),
                            asm_ty: AsmType::Long,
                        },
                        AsmInstr::Mov {
                            src: AsmOp::Reg(AsmReg::Ax),
                            dst: dst.codegen(ctx)?,
                            asm_ty,
                        },
                    ]),
                    AsmType::Long => Ok(vec![
                        AsmInstr::Cvttsd2si {
                            src: src.codegen(ctx)?,
                            dst: AsmOp::Reg(AsmReg::Ax),
                            asm_ty: AsmType::Quad,
                        },
                        AsmInstr::Mov {
                            src: AsmOp::Reg(AsmReg::Ax),
                            dst: dst.codegen(ctx)?,
                            asm_ty: AsmType::Long,
                        },
                    ]),
                    AsmType::Quad => {
                        let i64_ceil_as_u64 = (i64::MAX as u64) + 1;
                        let i64_ceil_as_f64 = i64_ceil_as_u64 as f64;

                        let static_const = StaticConst {
                            name: format!("static_const.{}", mktmp()),
                            init: StaticInit::Double(i64_ceil_as_f64),
                            ty: Type::Float64,
                            alignment: Type::alignment(Type::Float64),
                        };

                        ctx.static_consts.push(static_const.clone());

                        let tmp = mktmp();

                        let l1 = format!(".L{}_DoubleToUInt1_{tmp}", ctx.current_fn);
                        let l2 = format!(".L{}_DoubleToUInt2_{tmp}", ctx.current_fn);

                        Ok(vec![
                            AsmInstr::Comment("Convert Double to UInt64.".into()),
                            AsmInstr::Cmp {
                                src: AsmOp::Data(static_const.name.clone(), 0),
                                dst: src.codegen(ctx)?,
                                asm_ty: AsmType::Double,
                            },
                            AsmInstr::Jmpcc {
                                cc: ConditionCode::Ae,
                                target: l1.clone(),
                            },
                            AsmInstr::Cvttsd2si {
                                src: src.codegen(ctx)?,
                                dst: dst.codegen(ctx)?,
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Jmp { target: l2.clone() },
                            AsmInstr::Newline,
                            AsmInstr::Label(l1.clone()),
                            AsmInstr::Mov {
                                src: src.codegen(ctx)?,
                                dst: AsmOp::Reg(SSE_SCRATCH_1),
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Bin {
                                kind: AsmInstrBinKind::Sub,
                                lhs: AsmOp::Data(static_const.name.clone(), 0),
                                rhs: AsmOp::Reg(SSE_SCRATCH_1),
                                asm_ty: AsmType::Double,
                            },
                            AsmInstr::Cvttsd2si {
                                src: AsmOp::Reg(SSE_SCRATCH_1),
                                dst: dst.codegen(ctx)?,
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Bin {
                                kind: AsmInstrBinKind::Add,
                                lhs: AsmOp::Imm(i64_ceil_as_u64 as i64),
                                rhs: dst.codegen(ctx)?,
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Label(l2),
                        ])
                    }
                    _ => unreachable!(),
                }
            }

            IRInstr::IntToDouble {
                src,
                dst,
                src_ty,
                dst_ty,
            } => {
                let asm_ty = (*src_ty).into();

                match asm_ty {
                    AsmType::Byte | AsmType::Word => Ok(vec![
                        AsmInstr::Movsx {
                            src: src.codegen(ctx)?,
                            src_ty: asm_ty,
                            dst: AsmOp::Reg(AsmReg::Ax),
                            dst_ty: AsmType::Long,
                        },
                        AsmInstr::Cvtsi2sd {
                            src: AsmOp::Reg(AsmReg::Ax),
                            dst: dst.codegen(ctx)?,
                            asm_ty: AsmType::Long,
                        },
                    ]),
                    _ => Ok(vec![
                        AsmInstr::Comment(format!("Converting {src_ty:?} to {dst_ty:?}.")),
                        AsmInstr::Cvtsi2sd {
                            src: src.codegen(ctx)?,
                            dst: dst.codegen(ctx)?,
                            asm_ty,
                        },
                        AsmInstr::Newline,
                    ]),
                }
            }

            IRInstr::DoubleToFloat {
                src,
                src_ty: _,
                dst,
                dst_ty: _,
            } => Ok(vec![AsmInstr::Cvtsd2ss {
                src: src.codegen(ctx)?,
                dst: dst.codegen(ctx)?,
            }]),

            IRInstr::FloatToDouble {
                src,
                src_ty: _,
                dst,
                dst_ty: _,
            } => Ok(vec![AsmInstr::Cvtss2sd {
                src: src.codegen(ctx)?,
                dst: dst.codegen(ctx)?,
            }]),

            IRInstr::FloatToInt {
                src,
                src_ty: _,
                dst,
                dst_ty,
            } => {
                let asm_ty = (*dst_ty).into();

                match asm_ty {
                    AsmType::Byte | AsmType::Word => Ok(vec![
                        AsmInstr::Cvttss2si {
                            src: src.codegen(ctx)?,
                            dst: AsmOp::Reg(AsmReg::Ax),
                            asm_ty: AsmType::Long,
                        },
                        AsmInstr::Mov {
                            src: AsmOp::Reg(AsmReg::Ax),
                            dst: dst.codegen(ctx)?,
                            asm_ty,
                        },
                    ]),
                    AsmType::Long | AsmType::Quad => Ok(vec![AsmInstr::Cvttss2si {
                        src: src.codegen(ctx)?,
                        dst: dst.codegen(ctx)?,
                        asm_ty,
                    }]),
                    _ => unreachable!(),
                }
            }

            IRInstr::FloatToUInt {
                src,
                src_ty: _,
                dst,
                dst_ty,
            } => {
                let asm_ty = (*dst_ty).into();

                match asm_ty {
                    AsmType::Byte | AsmType::Word => Ok(vec![
                        AsmInstr::Cvttss2si {
                            src: src.codegen(ctx)?,
                            dst: AsmOp::Reg(AsmReg::Ax),
                            asm_ty: AsmType::Long,
                        },
                        AsmInstr::Mov {
                            src: AsmOp::Reg(AsmReg::Ax),
                            dst: dst.codegen(ctx)?,
                            asm_ty,
                        },
                    ]),
                    AsmType::Long => Ok(vec![
                        AsmInstr::Cvttss2si {
                            src: src.codegen(ctx)?,
                            dst: AsmOp::Reg(AsmReg::Ax),
                            asm_ty: AsmType::Quad,
                        },
                        AsmInstr::Mov {
                            src: AsmOp::Reg(AsmReg::Ax),
                            dst: dst.codegen(ctx)?,
                            asm_ty: AsmType::Long,
                        },
                    ]),
                    AsmType::Quad => {
                        let i64_ceil_as_u64 = (i64::MAX as u64) + 1;
                        let i64_ceil_as_f32 = i64_ceil_as_u64 as f32;

                        let static_const = StaticConst {
                            name: format!("static_const.{}", mktmp()),
                            init: StaticInit::Float(i64_ceil_as_f32),
                            ty: Type::Float32,
                            alignment: 4,
                        };

                        ctx.static_consts.push(static_const.clone());

                        let tmp = mktmp();

                        let l1 = format!(".L{}_FloatToUInt1_{tmp}", ctx.current_fn);
                        let l2 = format!(".L{}_FloatToUInt2_{tmp}", ctx.current_fn);

                        Ok(vec![
                            AsmInstr::Comment("Convert Float to UInt64.".into()),
                            AsmInstr::Cmp {
                                src: AsmOp::Data(static_const.name.clone(), 0),
                                dst: src.codegen(ctx)?,
                                asm_ty: AsmType::Float,
                            },
                            AsmInstr::Jmpcc {
                                cc: ConditionCode::Ae,
                                target: l1.clone(),
                            },
                            AsmInstr::Cvttss2si {
                                src: src.codegen(ctx)?,
                                dst: dst.codegen(ctx)?,
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Jmp { target: l2.clone() },
                            AsmInstr::Newline,
                            AsmInstr::Label(l1.clone()),
                            AsmInstr::Mov {
                                src: src.codegen(ctx)?,
                                dst: AsmOp::Reg(SSE_SCRATCH_1),
                                asm_ty: AsmType::Float,
                            },
                            AsmInstr::Bin {
                                kind: AsmInstrBinKind::Sub,
                                lhs: AsmOp::Data(static_const.name.clone(), 0),
                                rhs: AsmOp::Reg(SSE_SCRATCH_1),
                                asm_ty: AsmType::Float,
                            },
                            AsmInstr::Cvttss2si {
                                src: AsmOp::Reg(SSE_SCRATCH_1),
                                dst: dst.codegen(ctx)?,
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Bin {
                                kind: AsmInstrBinKind::Add,
                                lhs: AsmOp::Imm(i64_ceil_as_u64 as i64),
                                rhs: dst.codegen(ctx)?,
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Label(l2),
                        ])
                    }
                    _ => unreachable!(),
                }
            }

            IRInstr::IntToFloat {
                src,
                src_ty,
                dst,
                dst_ty: _,
            } => {
                let asm_ty = (*src_ty).into();

                match asm_ty {
                    AsmType::Byte | AsmType::Word => Ok(vec![
                        AsmInstr::Movsx {
                            src: src.codegen(ctx)?,
                            src_ty: asm_ty,
                            dst: AsmOp::Reg(AsmReg::Ax),
                            dst_ty: AsmType::Long,
                        },
                        AsmInstr::Cvtsi2ss {
                            src: AsmOp::Reg(AsmReg::Ax),
                            dst: dst.codegen(ctx)?,
                            asm_ty: AsmType::Long,
                        },
                    ]),
                    _ => Ok(vec![AsmInstr::Cvtsi2ss {
                        src: src.codegen(ctx)?,
                        dst: dst.codegen(ctx)?,
                        asm_ty,
                    }]),
                }
            }

            IRInstr::UIntToFloat {
                src,
                src_ty,
                dst,
                dst_ty: _,
            } => {
                let asm_ty = (*src_ty).into();

                match asm_ty {
                    AsmType::Byte | AsmType::Word => Ok(vec![
                        AsmInstr::Movzx {
                            src: src.codegen(ctx)?,
                            src_ty: asm_ty,
                            dst: AsmOp::Reg(AsmReg::Ax),
                            dst_ty: AsmType::Long,
                        },
                        AsmInstr::Cvtsi2ss {
                            src: AsmOp::Reg(AsmReg::Ax),
                            dst: dst.codegen(ctx)?,
                            asm_ty: AsmType::Long,
                        },
                    ]),
                    AsmType::Long => Ok(vec![
                        AsmInstr::Mov {
                            src: src.codegen(ctx)?,
                            dst: AsmOp::Reg(AsmReg::Ax),
                            asm_ty,
                        },
                        AsmInstr::Cvtsi2ss {
                            src: AsmOp::Reg(AsmReg::Ax),
                            dst: dst.codegen(ctx)?,
                            asm_ty: AsmType::Quad,
                        },
                    ]),
                    AsmType::Quad => {
                        let tmp = mktmp();

                        let l1 = format!(".L{}_UIntToFloat1_{tmp}", ctx.current_fn);
                        let l2 = format!(".L{}_UIntToFloat2_{tmp}", ctx.current_fn);

                        Ok(vec![
                            AsmInstr::Comment("Convert UInt64 to Float32.".into()),        
                            AsmInstr::Comment("Cvtsi2ss interprets its source as signed, so values >= 2^63 need special handling.".into()),
                            AsmInstr::Newline,
                            AsmInstr::Comment("Compare the source against zero using signed comparison.".into()),
                            AsmInstr::Cmp {
                                src: AsmOp::Imm(0),
                                dst: src.codegen(ctx)?,
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Newline,
                            AsmInstr::Comment("If the sign bit is set, jump to the unsigned conversion path.".into()),
                            AsmInstr::Jmpcc {
                                cc: ConditionCode::L,
                                target: l1.clone(),
                            },
                            AsmInstr::Newline,
                            AsmInstr::Comment("The value fits into a signed 64-bit integer.".into()),
                            AsmInstr::Comment("Convert it directly to double and skip the unsigned conversion path.".into()),
                            AsmInstr::Cvtsi2ss {
                                src: src.codegen(ctx)?,
                                dst: dst.codegen(ctx)?,
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Jmp { target: l2.clone() },
                            AsmInstr::Newline,
                            AsmInstr::Label(l1.clone()),
                            AsmInstr::Comment("The source is >= 2^63 and cannot be converted directly as a signed integer.".into()),
                            AsmInstr::Comment("Compute (src >> 1) | (src & 1), convert to double, then multiply by 2.".into()),
                            AsmInstr::Newline,
                            AsmInstr::Comment("Load the original value into %rax.".into()),
                            AsmInstr::Mov {
                                src: src.codegen(ctx)?,
                                dst: AsmOp::Reg(AsmReg::Ax),
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Newline,
                            AsmInstr::Comment("Copy the original value into %rdx.".into()),
                            AsmInstr::Mov {
                                src: AsmOp::Reg(AsmReg::Ax),
                                dst: AsmOp::Reg(AsmReg::Dx),
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Newline,
                            AsmInstr::Comment("Logically shift %rdx right by one bit.".into()),
                            AsmInstr::Comment("This clears the sign bit and divides the unsigned value by 2.".into()),
                            AsmInstr::Unary {
                                kind: AsmInstrUnaryKind::Shr,
                                dst: AsmOp::Reg(AsmReg::Dx),
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Newline,
                            AsmInstr::Comment("Extract the original least significant bit into %rax.".into()),
                            AsmInstr::Comment("This bit preserves rounding information lost by the right shift.".into()),
                            AsmInstr::Bin {
                                kind: AsmInstrBinKind::And,
                                lhs: AsmOp::Imm(1),
                                rhs: AsmOp::Reg(AsmReg::Ax),
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Newline,
                            AsmInstr::Comment("Combine the shifted value with the original least significant bit.".into()),
                            AsmInstr::Comment("%rdx = (src >> 1) | (src & 1).".into()),
                            AsmInstr::Bin {
                                kind: AsmInstrBinKind::Or,
                                lhs: AsmOp::Reg(AsmReg::Ax),
                                rhs: AsmOp::Reg(AsmReg::Dx),
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Newline,
                            AsmInstr::Comment("The adjusted value now fits into a signed 64-bit integer.".into()),
                            AsmInstr::Comment("Convert the adjusted value to double.".into()),
                            AsmInstr::Cvtsi2ss {
                                src: AsmOp::Reg(AsmReg::Dx),
                                dst: dst.codegen(ctx)?,
                                asm_ty: AsmType::Quad,
                            },
                            AsmInstr::Newline,
                            AsmInstr::Comment("Double the converted result to restore the original magnitude.".into()),
                            AsmInstr::Comment("Adding the double to itself is equivalent to multiplying by 2.".into()),
                            AsmInstr::Bin {
                                kind: AsmInstrBinKind::Add,
                                lhs: dst.codegen(ctx)?,
                                rhs: dst.codegen(ctx)?,
                                asm_ty: AsmType::Float,
                            },
                            AsmInstr::Label(l2),
                        ])
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
}

impl Codegen<AsmOp, CodegenContext> for IRVal {
    fn codegen(&self, ctx: &mut CodegenContext) -> Result<AsmOp, Box<dyn std::error::Error>> {
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
                Literal::Float32(_) | Literal::Float64(_) => {
                    Ok(AsmOp::Data(ctx.intern_float(*konst), 0))
                }
            },
            IRVal::Var { name, ty } => Ok(AsmOp::Pseudo {
                name: name.into(),
                ty: *ty,
            }),
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
    body.sink(AsmInstr::Comment("Establish a stack frame.".into()));
    body.sink(AsmInstr::Push(AsmOp::Reg(AsmReg::Bp)));
    body.sink(AsmInstr::Mov {
        src: AsmOp::Reg(AsmReg::Sp),
        dst: AsmOp::Reg(AsmReg::Bp),
        asm_ty: AsmType::Quad,
    });
    body.sink(AsmInstr::Newline);
    body.sink(AsmInstr::Comment("Reserve the stack space.".into()));
    body.sink(AsmInstr::Bin {
        kind: AsmInstrBinKind::Sub,
        lhs: AsmOp::Imm(0),
        rhs: AsmOp::Reg(AsmReg::Sp),
        asm_ty: AsmType::Quad,
    });
    body.sink(AsmInstr::Newline);
}

pub fn insert_epilogue<T>(body: &mut T)
where
    T: AsmSink,
{
    body.sink(AsmInstr::Comment(
        "Restore the previous stack frame.".into(),
    ));
    body.sink(AsmInstr::Mov {
        src: AsmOp::Reg(AsmReg::Bp),
        dst: AsmOp::Reg(AsmReg::Sp),
        asm_ty: AsmType::Quad,
    });
    body.sink(AsmInstr::Pop(AsmOp::Reg(AsmReg::Bp)));
    body.sink(AsmInstr::Newline);
}

impl CodegenContext {
    pub fn new() -> Self {
        Self {
            current_fn: String::new(),
            locals: HashMap::new(),
            funcs: HashMap::new(),
            static_consts: Vec::new(),
            float_pool: HashMap::new(),
        }
    }

    pub fn intern_float(&mut self, literal: Literal) -> String {
        let (key, ty, init) = match literal {
            Literal::Float32(v) => ((4, v.to_bits() as u64), Type::Float32, StaticInit::Float(v)),
            Literal::Float64(v) => ((8, v.to_bits()), Type::Float64, StaticInit::Double(v)),
            _ => unreachable!(),
        };

        if let Some(name) = self.float_pool.get(&key) {
            return name.clone();
        }

        let name = format!(".LC{}", mktmp());

        self.static_consts.push(StaticConst {
            name: name.clone(),
            ty,
            init,
            alignment: ty.alignment(),
        });

        name
    }
}
