use crate::asm::r#gen::{AsmFunc, AsmInstr, AsmOp, AsmProg, AsmReg};

pub fn emit(asm_prog: &AsmProg) {
    for f in &asm_prog.funcs {
        emit_fn(f);
    }
}

fn emit_fn(asm_fn: &AsmFunc) {
    for asm_instr in &asm_fn.body {
        match asm_instr {
            AsmInstr::Mov { src, dst } => {
                println!("movq {}, {}", src, dst);
            }

            AsmInstr::Ret => {
                println!("ret");
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
