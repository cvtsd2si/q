use crate::{
    asm::r#gen::{
        AsmFunc, AsmInstr, AsmInstrBinKind, AsmInstrShiftKind, AsmInstrUnaryKind, AsmOp, AsmProg,
        AsmReg,
    },
    parser::ast::Type,
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
            AsmInstr::Pop(val) => writeln!(f, "popq {}", val),
            AsmInstr::Push(val) => writeln!(f, "pushq {}", val),
            AsmInstr::Mov { src, dst } => writeln!(f, "movq {}, {}", src, dst),
            AsmInstr::Unary { kind, dst } => match kind {
                AsmInstrUnaryKind::Not => writeln!(f, "notq {dst}"),
                AsmInstrUnaryKind::Neg => writeln!(f, "negq {dst}"),
            },
            AsmInstr::Bin { kind, lhs, rhs } => match kind {
                AsmInstrBinKind::Add => writeln!(f, "addq {}, {}", lhs, rhs),
                AsmInstrBinKind::Sub => writeln!(f, "subq {}, {}", lhs, rhs),
            },
            AsmInstr::Cmp { src, dst } => writeln!(f, "cmpq {}, {}", src, dst),
            AsmInstr::Setcc { cc, dst } => match dst {
                AsmOp::Reg(reg) => writeln!(f, "set{cc} {}", reg.as_width(1)),
                AsmOp::Stack(_) | AsmOp::Mem(_, _) => writeln!(f, "set{cc} {dst}"),
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
            AsmInstr::CallIndirect { target } => writeln!(f, "call *{}", target.as_width(8)),
            AsmInstr::Lea { src, dst } => writeln!(f, "leaq {}, {}", src, dst),
            AsmInstr::Shift { kind, amount, dst } => match kind {
                AsmInstrShiftKind::Shl => writeln!(f, "shlq ${amount}, {dst}"),
                AsmInstrShiftKind::Shr => writeln!(f, "shrq ${amount}, {dst}"),
                AsmInstrShiftKind::Sar => writeln!(f, "sarq ${amount}, {dst}"),
            },
            AsmInstr::IntCast {
                src,
                src_ty,
                dst,
                dst_ty,
            } => {
                if !src_ty.is_int() || !dst_ty.is_int() {
                    return Err(std::fmt::Error);
                }

                let AsmOp::Reg(dst) = dst else {
                    return Err(std::fmt::Error);
                };

                let dst8 = dst.as_width(1);
                let dst16 = dst.as_width(2);
                let dst32 = dst.as_width(4);
                let dst64 = dst.as_width(8);

                /* Interpret src according to its original bit width and signedness.  */
                match src {
                    AsmOp::Imm(n) => {
                        if i32::try_from(*n).is_ok() {
                            writeln!(f, "movq ${n}, {dst64}")?;
                        } else {
                            writeln!(f, "movabsq ${n}, {dst64}")?;
                        }
                    }
                    _ => {
                        let src = cast_src(src, src_ty.size_of())?;

                        match src_ty {
                            Type::Int8 => {
                                writeln!(f, "movsbq {src}, {dst64}")?;
                            }
                            Type::UInt8 => {
                                writeln!(f, "movzbq {src}, {dst64}")?;
                            }
                            Type::Int16 => {
                                writeln!(f, "movswq {src}, {dst64}")?;
                            }
                            Type::UInt16 => {
                                writeln!(f, "movzwq {src}, {dst64}")?;
                            }
                            Type::Int32 => {
                                writeln!(f, "movzlq {src}, {dst64}")?;
                            }
                            Type::UInt32 => {
                                writeln!(f, "movl {src}, {dst32}")?;
                            }
                            Type::Int64 | Type::UInt64 => {
                                writeln!(f, "movq {src}, {dst64}")?;
                            }
                            _ => unreachable!(),
                        }
                    }
                }

                /* Convert to the destination width and signedness.  */
                match dst_ty {
                    Type::Int8 => {
                        writeln!(f, "\tmovabsq {dst8}, {dst64}")?;
                    }
                    Type::UInt8 => {
                        writeln!(f, "\tmovzbq {dst8}, {dst64}")?;
                    }
                    Type::Int16 => {
                        writeln!(f, "\tmovswq {dst16}, {dst64}")?;
                    }
                    Type::UInt16 => {
                        writeln!(f, "\tmovzwq {dst16}, {dst64}")?;
                    }
                    Type::Int32 => {
                        writeln!(f, "\tmovslq {dst32}, {dst64}")?;
                    }
                    Type::UInt32 => {
                        /* Writing to a 32-bit reg automatically clears the upper 32 bits.  */
                        writeln!(f, "\tmovl {dst32}, {dst32}")?;
                    }
                    /* Already 64 bits.  */
                    Type::Int64 | Type::UInt64 => {}
                    _ => unreachable!(),
                }

                writeln!(f)
            }
            AsmInstr::Comment(comment) => writeln!(f, "# {comment}"),
            AsmInstr::Newline => writeln!(f),
            _ => unreachable!(),
        }
    }
}

impl std::fmt::Display for AsmOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AsmOp::Reg(r) => write!(f, "{}", r.as_width(8)),
            AsmOp::Imm(n) => write!(f, "${}", n),
            AsmOp::Stack(offset) => write!(f, "{}(%rbp)", offset),
            AsmOp::Mem(reg, offset) => write!(f, "{offset}({})", reg.as_width(8)),
            _ => unreachable!("got: {:?}", self),
        }
    }
}

impl AsmReg {
    fn as_width(self, bytes: usize) -> &'static str {
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
        };

        match bytes {
            1 => names[0],
            2 => names[1],
            4 => names[2],
            8 => names[3],
            _ => unreachable!("Invalid register width: {bytes}"),
        }
    }
}

fn cast_src(src: &AsmOp, bytes: usize) -> Result<String, std::fmt::Error> {
    match src {
        AsmOp::Reg(reg) => Ok(reg.as_width(bytes).to_string()),
        AsmOp::Stack(_) | AsmOp::Mem(_, _) => Ok(src.to_string()),
        _ => Err(std::fmt::Error),
    }
}
