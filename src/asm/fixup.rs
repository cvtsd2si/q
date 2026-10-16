use crate::asm::r#gen::{AsmFunc, AsmInstr, AsmOp, AsmProg, AsmReg};

pub fn replace_pseudo(asm_prog: &mut AsmProg) -> &mut AsmProg {
    for f in &mut asm_prog.funcs {
        replace_pseudo_fn(f);
    }

    asm_prog
}

fn replace_pseudo_fn(asm_fn: &mut AsmFunc) {
    for asm_instr in &mut asm_fn.body {
        match asm_instr {
            AsmInstr::Mov { src, dst } => {
                replace_pseudo_op(src);
                replace_pseudo_op(dst);
            }

            AsmInstr::Ret | AsmInstr::Push(_) | AsmInstr::Pop(_) => {}
        }
    }
}

fn replace_pseudo_op(asm_op: &mut AsmOp) {
    if let AsmOp::Pseudo(pseudo) = asm_op {
        *asm_op = AsmOp::Reg(AsmReg::R9);
    }
}
