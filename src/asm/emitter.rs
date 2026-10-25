use crate::asm::r#gen::{
    AsmFunc, AsmInstr, AsmInstrBinKind, AsmInstrShiftKind, AsmInstrUnaryKind, AsmOp, AsmProg,
    AsmType,
};
use std::fmt::Write;

pub trait Emit {
    fn emit(&self, f: &mut String) -> Result<(), std::fmt::Error>;
}

impl Emit for AsmProg {
    fn emit(&self, f: &mut String) -> Result<(), std::fmt::Error> {
        for func in &self.funcs {
            func.emit(f)?;
        }

        Ok(())
    }
}

impl Emit for AsmFunc {
    fn emit(&self, f: &mut String) -> Result<(), std::fmt::Error> {
        writeln!(f, "\t.global {}", self.name)?;
        writeln!(f, "{}:", self.name)?;

        self.body.emit(f)?;

        Ok(())
    }
}

impl Emit for Vec<AsmInstr> {
    fn emit(&self, f: &mut String) -> Result<(), std::fmt::Error> {
        for instr in self {
            instr.emit(f)?;
        }

        Ok(())
    }
}

impl Emit for AsmInstr {
    fn emit(&self, f: &mut String) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self)
    }
}

impl std::fmt::Display for AsmInstr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let AsmInstr::Label(target) = self {
            return writeln!(f, "{}:", target);
        }

        write!(f, "\t")?;

        match self {
            AsmInstr::Ret => writeln!(f, "ret"),
            AsmInstr::Pop(val) => writeln!(f, "popq {}", val.as_width(AsmType::Quad)),
            AsmInstr::Push(val) => writeln!(f, "pushq {}", val.as_width(AsmType::Quad)),
            AsmInstr::Mov { src, dst, asm_ty } => {
                let is_src_xmm = src.clone().is_xmm();
                let is_dst_xmm = dst.clone().is_xmm();

                let mnemonic = match (is_src_xmm, is_dst_xmm, asm_ty.size()) {
                    /* GPR, immediate, or memory operands.  */
                    (false, false, 1) => "movb",
                    (false, false, 2) => "movw",
                    (false, false, 4) => "movl",
                    (false, false, 8) => "movq",

                    /* SSE -> SSE */
                    (true, true, 4) => "movss",
                    (true, true, 8) => "movsd",

                    /* SSE <-> GPR or memory.  */
                    (true, false, 4) | (false, true, 4) => "movd",
                    (true, false, 8) | (false, true, 8) => "movq",

                    _ => unreachable!("invalid mov: {src:?} -> {dst:?} ({asm_ty:?})"),
                };

                writeln!(
                    f,
                    "{mnemonic} {}, {}",
                    src.as_width(*asm_ty),
                    dst.as_width(*asm_ty)
                )
            }
            AsmInstr::Unary { kind, dst, asm_ty } => match kind {
                AsmInstrUnaryKind::Not => {
                    writeln!(f, "not{} {}", asm_ty.suffix(), dst.as_width(*asm_ty))
                }
                AsmInstrUnaryKind::Neg => {
                    writeln!(f, "neg{} {}", asm_ty.suffix(), dst.as_width(*asm_ty))
                }
                AsmInstrUnaryKind::Shr => {
                    writeln!(f, "shr{} {}", asm_ty.suffix(), dst.as_width(*asm_ty))
                }
            },
            AsmInstr::Bin {
                kind,
                lhs,
                rhs,
                asm_ty,
            } => match kind {
                AsmInstrBinKind::Add => writeln!(
                    f,
                    "add{} {}, {}",
                    asm_ty.suffix(),
                    lhs.as_width(*asm_ty),
                    rhs.as_width(*asm_ty)
                ),
                AsmInstrBinKind::Sub => writeln!(
                    f,
                    "sub{} {}, {}",
                    asm_ty.suffix(),
                    lhs.as_width(*asm_ty),
                    rhs.as_width(*asm_ty)
                ),
                AsmInstrBinKind::And => writeln!(
                    f,
                    "and{} {}, {}",
                    asm_ty.suffix(),
                    lhs.as_width(*asm_ty),
                    rhs.as_width(*asm_ty)
                ),
                AsmInstrBinKind::Or => writeln!(
                    f,
                    "or{} {}, {}",
                    asm_ty.suffix(),
                    lhs.as_width(*asm_ty),
                    rhs.as_width(*asm_ty)
                ),
            },
            AsmInstr::Cmp { src, dst, asm_ty } => writeln!(
                f,
                "cmp{} {}, {}",
                asm_ty.suffix(),
                src.as_width(*asm_ty),
                dst.as_width(*asm_ty)
            ),
            AsmInstr::Setcc { cc, dst } => match dst {
                AsmOp::Reg(reg) => writeln!(f, "set{cc} {}", reg.as_width(AsmType::Byte)),
                AsmOp::Stack(_) | AsmOp::Mem(_, _) => {
                    writeln!(f, "set{cc} {}", dst.as_width(AsmType::Byte))
                }
                _ => unreachable!("got: {dst:?}"),
            },
            AsmInstr::Jmpcc { cc, target } => writeln!(f, "j{cc} {target}"),
            AsmInstr::Jmp { target } => writeln!(f, "jmp {}", target),
            AsmInstr::Call { target } => writeln!(f, "call {}", target),
            AsmInstr::CallRt { target } => {
                let offset = target.offset();

                writeln!(f, "# Load up {target:?} from %r14 into %rax and call it.")?;
                writeln!(f, "\tmovq {offset}(%r14), %rax")?;
                writeln!(f, "\tcall *%rax")?;
                writeln!(f)?;
                writeln!(f, "\t# Zero-extend the retval.")?;
                writeln!(f, "\tmovl %eax, %eax")
            }
            AsmInstr::CallIndirect { target } => {
                writeln!(f, "call *{}", target.as_width(AsmType::Quad))
            }
            AsmInstr::Lea { src, dst } => writeln!(
                f,
                "leaq {}, {}",
                src.as_width(AsmType::Quad),
                dst.as_width(AsmType::Quad)
            ),
            AsmInstr::Shift {
                kind,
                amount,
                dst,
                asm_ty,
            } => match kind {
                AsmInstrShiftKind::Shl => writeln!(
                    f,
                    "shl{} ${amount}, {}",
                    asm_ty.suffix(),
                    dst.as_width(*asm_ty)
                ),
                AsmInstrShiftKind::Shr => writeln!(
                    f,
                    "shr{} ${amount}, {}",
                    asm_ty.suffix(),
                    dst.as_width(*asm_ty)
                ),
                AsmInstrShiftKind::Sar => writeln!(
                    f,
                    "sar{} ${amount}, {}",
                    asm_ty.suffix(),
                    dst.as_width(*asm_ty)
                ),
            },
            AsmInstr::Movsx {
                src,
                src_ty,
                dst,
                dst_ty,
            } => {
                let suffix = match (src_ty, dst_ty) {
                    (AsmType::Byte, AsmType::Word) => "bw",
                    (AsmType::Byte, AsmType::Long) => "bl",
                    (AsmType::Byte, AsmType::Quad) => "bq",
                    (AsmType::Word, AsmType::Long) => "wl",
                    (AsmType::Word, AsmType::Quad) => "wq",
                    (AsmType::Long, AsmType::Quad) => "lq",
                    _ => unreachable!("{src_ty:?}, {dst_ty:?}"),
                };

                writeln!(
                    f,
                    "movs{suffix} {}, {}",
                    src.as_width(*src_ty),
                    dst.as_width(*dst_ty)
                )
            }
            AsmInstr::Movzx {
                src,
                src_ty,
                dst,
                dst_ty,
            } => {
                let suffix = match (src_ty, dst_ty) {
                    (AsmType::Byte, AsmType::Word) => "bw",
                    (AsmType::Byte, AsmType::Long) => "bl",
                    (AsmType::Byte, AsmType::Quad) => "bq",
                    (AsmType::Word, AsmType::Long) => "wl",
                    (AsmType::Word, AsmType::Quad) => "wq",
                    (AsmType::Long, AsmType::Quad) => "lq",
                    _ => unreachable!("{src_ty:?}, {dst_ty:?}"),
                };

                writeln!(
                    f,
                    "movz{suffix} {}, {}",
                    src.as_width(*src_ty),
                    dst.as_width(*dst_ty)
                )
            }
            AsmInstr::Cvtsi2sd { src, dst, asm_ty } => {
                let suffix = match asm_ty {
                    AsmType::Long => "l",
                    AsmType::Quad => "q",
                    _ => unreachable!("got: {asm_ty:?}"),
                };

                writeln!(
                    f,
                    "cvtsi2sd{suffix} {}, {}",
                    src.as_width(*asm_ty),
                    dst.as_width(*asm_ty)
                )
            }
            AsmInstr::Comment(comment) => writeln!(f, "# {comment}"),
            AsmInstr::Newline => writeln!(f),
            _ => unreachable!(),
        }
    }
}
