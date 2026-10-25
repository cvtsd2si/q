use crate::asm::r#gen::{AsmFunc, AsmInstr, AsmOp, AsmProg, AsmReg, AsmType};

pub trait Fixup {
    type Output;

    fn fixup(&self) -> Self::Output;
}

impl Fixup for AsmProg {
    type Output = AsmProg;

    fn fixup(&self) -> Self::Output {
        AsmProg {
            funcs: self.funcs.iter().map(|f| f.fixup()).collect(),
            static_consts: self.static_consts.clone(),
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
            AsmInstr::Mov { src, dst, asm_ty }
                if matches!(src, AsmOp::Imm(_))
                    && (dst.clone().is_xmm()
                        || (asm_ty.size() == 8
                            && matches!(dst, AsmOp::Stack(_) | AsmOp::Mem(_, _)))) =>
            {
                vec![
                    AsmInstr::Mov {
                        src: src.clone(),
                        dst: AsmOp::Reg(AsmReg::R9),
                        asm_ty: *asm_ty,
                    },
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::R9),
                        dst: dst.clone(),
                        asm_ty: *asm_ty,
                    },
                ]
            }

            AsmInstr::Mov { src, dst, asm_ty }
                if matches!(src, AsmOp::Stack(_) | AsmOp::Mem(_, _) | AsmOp::Data(_, _))
                    && matches!(dst, AsmOp::Stack(_) | AsmOp::Mem(_, _)) =>
            {
                let scratch = match asm_ty {
                    AsmType::Float | AsmType::Double => AsmReg::Xmm15,
                    AsmType::Byte | AsmType::Word | AsmType::Long | AsmType::Quad => AsmReg::R9,
                };

                vec![
                    AsmInstr::Mov {
                        src: src.clone(),
                        dst: AsmOp::Reg(scratch),
                        asm_ty: *asm_ty,
                    },
                    AsmInstr::Mov {
                        src: AsmOp::Reg(scratch),
                        dst: dst.clone(),
                        asm_ty: *asm_ty,
                    },
                ]
            }

            AsmInstr::Bin {
                kind,
                lhs,
                rhs,
                asm_ty,
            } if matches!(asm_ty, AsmType::Float | AsmType::Double)
                && matches!(rhs, AsmOp::Stack(_) | AsmOp::Mem(_, _)) =>
            {
                vec![
                    AsmInstr::Mov {
                        src: rhs.clone(),
                        dst: AsmOp::Reg(AsmReg::Xmm15),
                        asm_ty: *asm_ty,
                    },
                    AsmInstr::Bin {
                        kind: *kind,
                        lhs: lhs.clone(),
                        rhs: AsmOp::Reg(AsmReg::Xmm15),
                        asm_ty: *asm_ty,
                    },
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::Xmm15),
                        dst: rhs.clone(),
                        asm_ty: *asm_ty,
                    },
                ]
            }

            AsmInstr::Bin {
                kind,
                lhs,
                rhs,
                asm_ty,
            } if matches!(lhs, AsmOp::Stack(_) | AsmOp::Mem(_, _))
                && matches!(rhs, AsmOp::Stack(_) | AsmOp::Mem(_, _)) =>
            {
                vec![
                    AsmInstr::Mov {
                        src: lhs.clone(),
                        dst: AsmOp::Reg(AsmReg::R9),
                        asm_ty: *asm_ty,
                    },
                    AsmInstr::Bin {
                        kind: *kind,
                        lhs: AsmOp::Reg(AsmReg::R9),
                        rhs: rhs.clone(),
                        asm_ty: *asm_ty,
                    },
                ]
            }

            AsmInstr::Cmp { src, dst, asm_ty } if matches!(dst, AsmOp::Imm(_)) => vec![
                AsmInstr::Mov {
                    src: dst.clone(),
                    dst: AsmOp::Reg(AsmReg::R9),
                    asm_ty: *asm_ty,
                },
                AsmInstr::Cmp {
                    src: src.clone(),
                    dst: AsmOp::Reg(AsmReg::R9),
                    asm_ty: *asm_ty,
                },
            ],

            AsmInstr::Cvtsi2sd { src, dst, asm_ty }
                if matches!(dst, AsmOp::Stack(_) | AsmOp::Mem(_, _)) =>
            {
                vec![
                    AsmInstr::Cvtsi2sd {
                        src: src.clone(),
                        dst: AsmOp::Reg(AsmReg::Xmm15),
                        asm_ty: *asm_ty,
                    },
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::Xmm15),
                        dst: dst.clone(),
                        asm_ty: AsmType::Double,
                    },
                ]
            }

            _ => vec![self.clone()],
        }
    }
}
