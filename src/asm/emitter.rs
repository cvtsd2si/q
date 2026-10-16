use crate::asm::r#gen::{AsmFunc, AsmInstr, AsmOp, AsmProg, AsmReg};

pub trait Emit {
    fn emit(&self);
}

impl Emit for AsmProg {
    fn emit(&self) {
        for f in &self.funcs {
            f.emit();
        }
    }
}

impl Emit for AsmFunc {
    fn emit(&self) {
        println!("\t.global {}", self.name);
        println!("{}:", self.name);

        for instr in &self.body {
            instr.emit();
        }
    }
}

impl Emit for AsmInstr {
    fn emit(&self) {
        print!("\t");

        match self {
            AsmInstr::Mov { src, dst } => {
                println!("movq {}, {}", src, dst);
            }

            AsmInstr::Ret => {
                println!("ret");
            }

            AsmInstr::Push(reg) => {
                println!("pushq {}", reg);
            }

            AsmInstr::Pop(reg) => {
                println!("popq {}", reg);
            }
        }
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
