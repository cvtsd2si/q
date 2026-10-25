use crate::asm::r#gen::{AsmFunc, AsmInstr, AsmOp, AsmProg, AsmReg};

pub trait Fixup {
    type Output;

    fn fixup(&self) -> Self::Output;
}

impl Fixup for AsmProg {
    type Output = AsmProg;

    fn fixup(&self) -> Self::Output {
        AsmProg {
            funcs: self.funcs.iter().map(|f| f.fixup()).collect(),
        }
    }
}

impl Fixup for AsmFunc {
    type Output = AsmFunc;
    fn fixup(&self) -> Self::Output {
        AsmFunc {
            name: self.name.clone(),
            body: self.body.iter().flat_map(|instr| instr.fixup()).collect(),
            space: self.space,
        }
    }
}

impl Fixup for AsmInstr {
    type Output = Vec<AsmInstr>;

    fn fixup(&self) -> Self::Output {
        match self {
            AsmInstr::Mov { src, dst }
                if matches!(src, AsmOp::Stack(_) | AsmOp::Mem(_, _))
                    && matches!(dst, AsmOp::Stack(_) | AsmOp::Mem(_, _)) =>
            {
                vec![
                    AsmInstr::Mov {
                        src: src.clone(),
                        dst: AsmOp::Reg(AsmReg::R9),
                    },
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::R9),
                        dst: dst.clone(),
                    },
                ]
            }

            AsmInstr::Bin { kind, lhs, rhs }
                if matches!(lhs, AsmOp::Stack(_) | AsmOp::Mem(_, _))
                    && matches!(rhs, AsmOp::Stack(_) | AsmOp::Mem(_, _)) =>
            {
                vec![
                    AsmInstr::Mov {
                        src: lhs.clone(),
                        dst: AsmOp::Reg(AsmReg::R9),
                    },
                    AsmInstr::Bin {
                        kind: *kind,
                        lhs: AsmOp::Reg(AsmReg::R9),
                        rhs: rhs.clone(),
                    },
                ]
            }

            AsmInstr::Cmp { src, dst } if matches!(dst, AsmOp::Imm(_)) => vec![
                AsmInstr::Mov {
                    src: dst.clone(),
                    dst: AsmOp::Reg(AsmReg::R9),
                },
                AsmInstr::Cmp {
                    src: src.clone(),
                    dst: AsmOp::Reg(AsmReg::R9),
                },
            ],

            AsmInstr::IntCast {
                src,
                src_ty,
                dst,
                dst_ty,
            } if matches!(dst, AsmOp::Stack(_) | AsmOp::Mem(_, _)) => vec![
                AsmInstr::IntCast {
                    src: src.clone(),
                    src_ty: *src_ty,
                    dst: AsmOp::Reg(AsmReg::R9),
                    dst_ty: *dst_ty,
                },
                AsmInstr::Mov {
                    src: AsmOp::Reg(AsmReg::R9),
                    dst: dst.clone(),
                },
            ],

            _ => vec![self.clone()],
        }
    }
}
