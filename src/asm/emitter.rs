use crate::asm::r#gen::{AsmFunc, AsmInstr, AsmInstrBinKind, AsmOp, AsmProg, AsmReg};
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

        writeln!(f)?;

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
            AsmInstr::Bin { kind, lhs, rhs } => match kind {
                AsmInstrBinKind::Add => writeln!(f, "addq {}, {}", lhs, rhs),
                AsmInstrBinKind::Sub => writeln!(f, "subq {}, {}", lhs, rhs),
            },
            AsmInstr::Cmp { src, dst } => writeln!(f, "cmpq {}, {}", src, dst),
            AsmInstr::Jz { target } => writeln!(f, "je {}", target),
            AsmInstr::Jnz { target } => writeln!(f, "jne {}", target),
            AsmInstr::Jmp { target } => writeln!(f, "jmp {}", target),
            AsmInstr::Call { target } => writeln!(f, "call {}", target),
            AsmInstr::CallRt { target } => {
                let offset = target.offset();

                writeln!(f, "movq {offset}(%r14), %rax")?;
                writeln!(f, "\tcall *%rax")?;

                // Zero-extend the retval
                writeln!(f, "\tmovl %eax, %eax")
            }
            AsmInstr::CallIndirect { target } => writeln!(f, "call *{}", target),
            _ => unreachable!(),
        }
    }
}

impl std::fmt::Display for AsmOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AsmOp::Reg(r) => write!(f, "{r}"),
            AsmOp::Imm(n) => write!(f, "${}", n),
            AsmOp::Stack(offset) => write!(f, "{}(%rbp)", offset),
            AsmOp::Mem(reg, offset) => write!(f, "{offset}({})", reg),
            _ => unreachable!("got: {:?}", self),
        }
    }
}

impl std::fmt::Display for AsmReg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AsmReg::Bp => write!(f, "%rbp"),
            AsmReg::Sp => write!(f, "%rsp"),
            AsmReg::Ax => write!(f, "%rax"),
            AsmReg::R8 => write!(f, "%r8"),
            AsmReg::R9 => write!(f, "%r9"),
            AsmReg::R10 => write!(f, "%r10"),
            AsmReg::R11 => write!(f, "%r11"),
            AsmReg::R12 => write!(f, "%r12"),
            AsmReg::R13 => write!(f, "%r13"),
            AsmReg::R14 => write!(f, "%r14"),
            AsmReg::Cx => write!(f, "%rcx"),
            AsmReg::Dx => write!(f, "%rdx"),
            AsmReg::Si => write!(f, "%rsi"),
            AsmReg::Di => write!(f, "%rdi"),
        }
    }
}
