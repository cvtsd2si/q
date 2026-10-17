use crate::asm::r#gen::{AsmFunc, AsmInstr, AsmOp, AsmProg, AsmReg};

pub trait ReplacePseudo {
    fn replace_pseudo(&mut self) -> &mut Self;
}

impl ReplacePseudo for AsmProg {
    fn replace_pseudo(&mut self) -> &mut Self {
        for f in &mut self.funcs {
            f.replace_pseudo();
        }

        self
    }
}

impl ReplacePseudo for AsmFunc {
    fn replace_pseudo(&mut self) -> &mut Self {
        for instr in &mut self.body {
            instr.replace_pseudo();
        }

        self
    }
}

impl ReplacePseudo for AsmInstr {
    fn replace_pseudo(&mut self) -> &mut Self {
        match self {
            AsmInstr::Mov { src, dst } => {
                src.replace_pseudo();
                dst.replace_pseudo();
            }

            AsmInstr::Ret | AsmInstr::Push(_) | AsmInstr::Pop(_) => {}
            
            AsmInstr::Bin { kind: _, lhs, rhs } => {
                lhs.replace_pseudo();
                rhs.replace_pseudo();
            }

        }

        self
    }
}

impl ReplacePseudo for AsmOp {
    fn replace_pseudo(&mut self) -> &mut Self {
        if let AsmOp::Pseudo(pseudo) = self {
            *self = AsmOp::Reg(AsmReg::R9);
        }

        self
    }
}
