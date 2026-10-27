use crate::{
    asm::r#gen::{
        AsmFunc, AsmInstr, AsmInstrBinKind, AsmInstrShiftKind, AsmInstrUnaryKind, AsmOp, AsmProg,
        AsmStaticConst, AsmType,
    },
    ir::r#gen::StaticInit,
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

        for konst in &self.static_consts {
            konst.emit(f)?;
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

impl Emit for AsmStaticConst {
    fn emit(&self, f: &mut String) -> Result<(), std::fmt::Error> {
        writeln!(f)?;

        writeln!(f, "\t.balign 0x{:x}", self.alignment)?;
        writeln!(f, "{}:", self.name)?;

        match self.init {
            StaticInit::Float(value) => writeln!(f, "\t.long 0x{:08x}", value.to_bits()),
            StaticInit::Double(value) => writeln!(f, "\t.quad 0x{:016x}", value.to_bits()),
        }
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
            AsmInstr::Cmp { src, dst, asm_ty } => {
                let instr = match asm_ty {
                    AsmType::Byte => "cmpb",
                    AsmType::Word => "cmpw",
                    AsmType::Long => "cmpl",
                    AsmType::Quad => "cmpq",
                    AsmType::Float => "comiss",
                    AsmType::Double => "comisd",
                };
                writeln!(
                    f,
                    "{instr} {}, {}",
                    src.as_width(*asm_ty),
                    dst.as_width(*asm_ty)
                )
            }
            AsmInstr::Setcc { cc, dst } => match dst {
                AsmOp::Reg(reg) => writeln!(f, "set{cc} {}", reg.as_width(AsmType::Byte)),
                AsmOp::Mem(_, _) => {
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
                writeln!(f, "\tmovq 0x{offset:x}(%r14), %rax")?;
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
                    "shl{} $0x{amount:x}, {}",
                    asm_ty.suffix(),
                    dst.as_width(*asm_ty)
                ),
                AsmInstrShiftKind::Shr => writeln!(
                    f,
                    "shr{} $0x{amount:x}, {}",
                    asm_ty.suffix(),
                    dst.as_width(*asm_ty)
                ),
                AsmInstrShiftKind::Sar => writeln!(
                    f,
                    "sar{} $0x{amount:x}, {}",
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
                    _ => unreachable!("{src_ty:?}, {dst_ty:?}"),
                };

                writeln!(
                    f,
                    "movz{suffix} {}, {}",
                    src.as_width(*src_ty),
                    dst.as_width(*dst_ty)
                )
            }
            AsmInstr::Cvtsi2sd { src, dst, asm_ty }
            | AsmInstr::Cvtsi2ss { src, dst, asm_ty }
            | AsmInstr::Cvttsd2si { src, dst, asm_ty }
            | AsmInstr::Cvttss2si { src, dst, asm_ty } => {
                let mnemonic = match self {
                    AsmInstr::Cvtsi2sd { .. } => "cvtsi2sd",
                    AsmInstr::Cvtsi2ss { .. } => "cvtsi2ss",
                    AsmInstr::Cvttsd2si { .. } => "cvttsd2si",
                    AsmInstr::Cvttss2si { .. } => "cvttss2si",
                    _ => unreachable!(),
                };

                let suffix = match asm_ty {
                    AsmType::Long => "l",
                    AsmType::Quad => "q",
                    _ => unreachable!(),
                };

                writeln!(
                    f,
                    "{mnemonic}{suffix} {}, {}",
                    src.as_width(*asm_ty),
                    dst.as_width(*asm_ty)
                )
            }
            AsmInstr::Cvtss2sd { src, dst } => writeln!(
                f,
                "cvtss2sd {}, {}",
                src.as_width(AsmType::Float),
                dst.as_width(AsmType::Double)
            ),

            AsmInstr::Cvtsd2ss { src, dst } => writeln!(
                f,
                "cvtsd2ss {}, {}",
                src.as_width(AsmType::Double),
                dst.as_width(AsmType::Float)
            ),
            AsmInstr::Test {
                src,
                dst: _,
                asm_ty,
            } => writeln!(
                f,
                "test{} {}, {}",
                asm_ty.suffix(),
                src.as_width(*asm_ty),
                src.as_width(*asm_ty),
            ),
            AsmInstr::Comment(comment) => writeln!(f, "# {comment}"),
            AsmInstr::Newline => writeln!(f),
            _ => unreachable!(),
        }
    }
}
