use crate::asm::r#gen::{AsmFunc, AsmInstr, AsmOp, AsmProg, AsmReg};
use std::{fs::File, io::Write};

pub trait Emit {
    fn emit(&self, f: &mut File) -> Result<(), std::io::Error>;
}

impl Emit for AsmProg {
    fn emit(&self, f: &mut File) -> Result<(), std::io::Error> {
        for func in &self.funcs {
            func.emit(f)?;
        }

        Ok(())
    }
}

impl Emit for AsmFunc {
    fn emit(&self, f: &mut File) -> Result<(), std::io::Error> {
        writeln!(f, "\t.global {}", self.name)?;
        writeln!(f, "{}:", self.name)?;

        for instr in &self.body {
            instr.emit(f)?;
        }

        Ok(())
    }
}

impl Emit for AsmInstr {
    fn emit(&self, f: &mut File) -> Result<(), std::io::Error> {
        write!(f, "\t")?;

        match self {
            AsmInstr::Mov { src, dst } => {
                writeln!(f, "movq {}, {}", src, dst)?;
            }

            AsmInstr::Ret => {
                writeln!(f, "ret")?;
            }

            AsmInstr::Push(reg) => {
                writeln!(f, "pushq {}", reg)?;
            }

            AsmInstr::Pop(reg) => {
                writeln!(f, "popq {}", reg)?;
            }
        }

        Ok(())
    }
}

impl std::fmt::Display for AsmOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AsmOp::Reg(r) => match r {
                AsmReg::Bp => write!(f, "%rbp"),
                AsmReg::Sp => write!(f, "%rsp"),
                AsmReg::Ax => write!(f, "%rax"),
                AsmReg::R9 => write!(f, "%r9"),
            },

            AsmOp::Imm(n) => write!(f, "${}", n),

            _ => unreachable!(),
        }
    }
}
