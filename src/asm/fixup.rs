use crate::asm::r#gen::{AsmFunc, AsmInstr, AsmOp, AsmProg, AsmReg, AsmType};

pub trait Fixup {
    type Output;
    fn fixup(self) -> Self::Output;
}

impl Fixup for AsmProg {
    type Output = AsmProg;

    fn fixup(self) -> Self::Output {
        AsmProg {
            funcs: self.funcs.into_iter().map(|f| f.fixup()).collect(),
            static_consts: self.static_consts,
        }
    }
}

impl Fixup for AsmFunc {
    type Output = AsmFunc;

    fn fixup(self) -> Self::Output {
        AsmFunc {
            name: self.name,
            body: self
                .body
                .into_iter()
                .flat_map(|instr| instr.fixup())
                .collect(),
            space: self.space,
        }
    }
}

impl Fixup for AsmInstr {
    type Output = Vec<AsmInstr>;

    fn fixup(self) -> Self::Output {
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
                        asm_ty,
                    },
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::R9),
                        dst: dst.clone(),
                        asm_ty,
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
                        asm_ty,
                    },
                    AsmInstr::Mov {
                        src: AsmOp::Reg(scratch),
                        dst: dst.clone(),
                        asm_ty,
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
                        asm_ty,
                    },
                    AsmInstr::Bin {
                        kind,
                        lhs: lhs.clone(),
                        rhs: AsmOp::Reg(AsmReg::Xmm15),
                        asm_ty,
                    },
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::Xmm15),
                        dst: rhs.clone(),
                        asm_ty,
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
                        asm_ty,
                    },
                    AsmInstr::Bin {
                        kind,
                        lhs: AsmOp::Reg(AsmReg::R9),
                        rhs: rhs.clone(),
                        asm_ty,
                    },
                ]
            }

            AsmInstr::Cmp {
                asm_ty: AsmType::Float | AsmType::Double,
                src: _,
                dst: AsmOp::Reg(_),
            } => vec![self.clone()],

            AsmInstr::Cmp { asm_ty, src, dst }
                if matches!(asm_ty, AsmType::Float | AsmType::Double) =>
            {
                vec![
                    AsmInstr::Mov {
                        asm_ty,
                        src: dst.clone(),
                        dst: AsmOp::Reg(AsmReg::Xmm15),
                    },
                    AsmInstr::Cmp {
                        asm_ty,
                        src: src.clone(),
                        dst: AsmOp::Reg(AsmReg::Xmm15),
                    },
                ]
            }

            /* Not both operands of 'cmp' can be in memory */
            AsmInstr::Cmp { asm_ty, src, dst }
                if matches!(src, AsmOp::Mem(_, _) | AsmOp::Data(_, _))
                    && matches!(dst, AsmOp::Mem(_, _) | AsmOp::Data(_, _)) =>
            {
                vec![
                    AsmInstr::Mov {
                        asm_ty,
                        src: src.clone(),
                        dst: AsmOp::Reg(AsmReg::R9),
                    },
                    AsmInstr::Cmp {
                        asm_ty,
                        src: AsmOp::Reg(AsmReg::R9),
                        dst: dst.clone(),
                    },
                ]
            }

            /* First operand of 'cmp' can't be a large constant, and second can't be a constant at all */
            AsmInstr::Cmp {
                asm_ty: AsmType::Quad,
                src: AsmOp::Imm(imm1),
                dst: AsmOp::Imm(imm2),
            } if is_large(imm1) => {
                vec![
                    AsmInstr::Mov {
                        asm_ty: AsmType::Quad,
                        src: AsmOp::Imm(imm1),
                        dst: AsmOp::Reg(AsmReg::R9),
                    },
                    AsmInstr::Mov {
                        asm_ty: AsmType::Quad,
                        src: AsmOp::Imm(imm2),
                        dst: AsmOp::Reg(AsmReg::R10),
                    },
                    AsmInstr::Cmp {
                        asm_ty: AsmType::Quad,
                        src: AsmOp::Reg(AsmReg::R9),
                        dst: AsmOp::Reg(AsmReg::R10),
                    },
                ]
            }

            AsmInstr::Cmp {
                asm_ty: AsmType::Quad,
                src: AsmOp::Imm(imm1),
                dst,
            } if is_large(imm1) => {
                vec![
                    AsmInstr::Mov {
                        asm_ty: AsmType::Quad,
                        src: AsmOp::Imm(imm1),
                        dst: AsmOp::Reg(AsmReg::R9),
                    },
                    AsmInstr::Cmp {
                        asm_ty: AsmType::Quad,
                        src: AsmOp::Reg(AsmReg::R9),
                        dst: dst.clone(),
                    },
                ]
            }

            AsmInstr::Cmp {
                asm_ty,
                src,
                dst: AsmOp::Imm(imm),
            } => {
                vec![
                    AsmInstr::Mov {
                        asm_ty,
                        src: AsmOp::Imm(imm),
                        dst: AsmOp::Reg(AsmReg::R9),
                    },
                    AsmInstr::Cmp {
                        asm_ty,
                        src: src.clone(),
                        dst: AsmOp::Reg(AsmReg::R9),
                    },
                ]
            }

            AsmInstr::Cvtsi2sd { src, dst, asm_ty }
                if matches!(dst, AsmOp::Stack(_) | AsmOp::Mem(_, _)) =>
            {
                vec![
                    AsmInstr::Cvtsi2sd {
                        src: src.clone(),
                        dst: AsmOp::Reg(AsmReg::Xmm15),
                        asm_ty,
                    },
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::Xmm15),
                        dst: dst.clone(),
                        asm_ty: AsmType::Double,
                    },
                ]
            }

            AsmInstr::Cvtsi2ss { src, dst, asm_ty }
                if matches!(dst, AsmOp::Stack(_) | AsmOp::Mem(_, _)) =>
            {
                vec![
                    AsmInstr::Cvtsi2ss {
                        src: src.clone(),
                        dst: AsmOp::Reg(AsmReg::Xmm15),
                        asm_ty,
                    },
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::Xmm15),
                        dst: dst.clone(),
                        asm_ty: AsmType::Double,
                    },
                ]
            }

            AsmInstr::Cvttsd2si { src, dst, asm_ty }
                if matches!(dst, AsmOp::Stack(_) | AsmOp::Mem(_, _)) =>
            {
                vec![
                    AsmInstr::Cvttsd2si {
                        src: src.clone(),
                        dst: AsmOp::Reg(AsmReg::Xmm15),
                        asm_ty,
                    },
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::Xmm15),
                        dst: dst.clone(),
                        asm_ty: AsmType::Double,
                    },
                ]
            }

            AsmInstr::Cvttss2si { src, dst, asm_ty }
                if matches!(dst, AsmOp::Stack(_) | AsmOp::Mem(_, _)) =>
            {
                vec![
                    AsmInstr::Cvttss2si {
                        src: src.clone(),
                        dst: AsmOp::Reg(AsmReg::Xmm15),
                        asm_ty,
                    },
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::Xmm15),
                        dst: dst.clone(),
                        asm_ty: AsmType::Double,
                    },
                ]
            }

            AsmInstr::Cvtss2sd { src, dst }
                if matches!(dst, AsmOp::Stack(_) | AsmOp::Mem(_, _)) =>
            {
                vec![
                    AsmInstr::Cvtss2sd {
                        src: src.clone(),
                        dst: AsmOp::Reg(AsmReg::Xmm15),
                    },
                    AsmInstr::Mov {
                        src: AsmOp::Reg(AsmReg::Xmm15),
                        dst: dst.clone(),
                        asm_ty: AsmType::Double,
                    },
                ]
            }

            AsmInstr::Cvtsd2ss { src, dst }
                if matches!(dst, AsmOp::Stack(_) | AsmOp::Mem(_, _)) =>
            {
                vec![
                    AsmInstr::Cvtsd2ss {
                        src: src.clone(),
                        dst: AsmOp::Reg(AsmReg::Xmm15),
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

fn is_large(n: i64) -> bool {
    n > i32::MAX as i64 || n < i32::MIN as i64
}
