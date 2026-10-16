use crate::asm::r#gen::{AsmInstr, AsmOp, AsmProg, AsmReg};

pub fn fixup(asm_prog: &mut AsmProg) -> &mut AsmProg {
    for asm_instr in &mut asm_prog.instrs {
        match asm_instr {
            AsmInstr::Mov { src, dst } => {
                replace_pseudo(src);
                replace_pseudo(dst);
            }
            AsmInstr::Ret => {}
        }
    }

    asm_prog
}

fn replace_pseudo(asm_op: &mut AsmOp) {
    if let AsmOp::Pseudo(pseudo) = asm_op {
        *asm_op = AsmOp::Reg(AsmReg::R9);
    }
}
