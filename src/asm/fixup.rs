use crate::asm::r#gen::{AsmFunc, AsmInstr, AsmOp, AsmProg, AsmReg, AsmType};
use std::collections::VecDeque;

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
        let mut pending = VecDeque::from([self]);
        let mut output = Vec::new();

        while let Some(instr) = pending.pop_front() {
            match instr.clone().rewrite_once() {
                // FIXME: don't clone
                Ok(replacements) => {
                    if cfg!(debug_assertions) {
                        eprintln!("rewritten {instr:?} to {replacements:?}");
                    }

                    for replacement in replacements.into_iter().rev() {
                        pending.push_front(replacement.to_owned());
                    }
                }
                Err(instr) => {
                    output.push(instr.to_owned());
                }
            }
        }

        output
    }
}

impl AsmInstr {
    fn rewrite_once(self) -> Result<Vec<AsmInstr>, AsmInstr> {
        match self {
            /* 'mov' cannot move a value from one mem addr to another.  */
            AsmInstr::Mov { src, dst, asm_ty } if is_mem(&src) && is_mem(&dst) => {
                let scratch = match asm_ty {
                    AsmType::Float | AsmType::Double => AsmReg::Xmm15,
                    AsmType::Byte | AsmType::Word | AsmType::Long | AsmType::Quad => AsmReg::R9,
                };

                Ok(vec![
                    AsmInstr::Mov {
                        src,
                        dst: AsmOp::Reg(scratch),
                        asm_ty,
                    },
                    AsmInstr::Mov {
                        src: AsmOp::Reg(scratch),
                        dst,
                        asm_ty,
                    },
                ])
            }

            /* 'movq' cannot move a large constant to a mem address.  */
            AsmInstr::Mov {
                src: AsmOp::Imm(n),
                dst,
                asm_ty: AsmType::Quad,
            } if is_large(n) && is_mem(&dst) => Ok(vec![
                AsmInstr::Mov {
                    src: AsmOp::Imm(n),
                    dst: AsmOp::Reg(AsmReg::R9),
                    asm_ty: AsmType::Quad,
                },
                AsmInstr::Mov {
                    src: AsmOp::Reg(AsmReg::R9),
                    dst,
                    asm_ty: AsmType::Quad,
                },
            ]),

            /* 'mov' cannot move any constant to an SSE reg.  */
            AsmInstr::Mov {
                src: AsmOp::Imm(n),
                dst,
                asm_ty: _,
            } if dst.is_xmm() => Ok(vec![
                AsmInstr::Mov {
                    src: AsmOp::Imm(n),
                    dst: AsmOp::Reg(AsmReg::R9),
                    asm_ty: AsmType::Quad,
                },
                AsmInstr::Mov {
                    src: AsmOp::Reg(AsmReg::R9),
                    dst,
                    asm_ty: AsmType::Quad,
                },
            ]),

            /* 'movzx' src cannot be a constant.  */
            AsmInstr::Movzx {
                src: AsmOp::Imm(n),
                src_ty,
                dst,
                dst_ty,
            } => Ok(vec![
                AsmInstr::Mov {
                    src: AsmOp::Imm(n),
                    dst: AsmOp::Reg(AsmReg::R9),
                    asm_ty: src_ty,
                },
                AsmInstr::Movzx {
                    src: AsmOp::Reg(AsmReg::R9),
                    src_ty,
                    dst,
                    dst_ty,
                },
            ]),

            /* 'movzb{} | movzw{}' are illegal, if dst is mem.  */
            AsmInstr::Movzx {
                src_ty,
                src,
                dst_ty,
                dst,
            } if matches!(src_ty, AsmType::Byte | AsmType::Word) && is_mem(&dst) => Ok(vec![
                AsmInstr::Movzx {
                    src_ty,
                    src,
                    dst_ty,
                    dst: AsmOp::Reg(AsmReg::R9),
                },
                AsmInstr::Mov {
                    asm_ty: dst_ty,
                    src: AsmOp::Reg(AsmReg::R9),
                    dst,
                },
            ]),

            /* 'movzlq' does not exist.  Zero-extend with 'movl' before
             * storing with 'movq', if dst is mem.  */
            AsmInstr::Movzx {
                src,
                src_ty: AsmType::Long,
                dst,
                dst_ty: AsmType::Quad,
            } if is_mem(&dst) => Ok(vec![
                AsmInstr::Mov {
                    src,
                    dst: AsmOp::Reg(AsmReg::R9),
                    asm_ty: AsmType::Long,
                },
                AsmInstr::Mov {
                    src: AsmOp::Reg(AsmReg::R9),
                    dst,
                    asm_ty: AsmType::Quad,
                },
            ]),

            /* 'movzlq' does not exist.  'movl' implicitly zero-extends to 64 bits.  */
            AsmInstr::Movzx {
                src,
                src_ty: AsmType::Long,
                dst,
                dst_ty: AsmType::Quad,
            } => Ok(vec![AsmInstr::Mov {
                src,
                dst,
                asm_ty: AsmType::Long,
            }]),

            /* 'movsx' src cannot be a constant.  */
            AsmInstr::Movsx {
                src: AsmOp::Imm(n),
                src_ty,
                dst,
                dst_ty,
            } => Ok(vec![
                AsmInstr::Mov {
                    src: AsmOp::Imm(n),
                    dst: AsmOp::Reg(AsmReg::R9),
                    asm_ty: src_ty,
                },
                AsmInstr::Movsx {
                    src: AsmOp::Reg(AsmReg::R9),
                    src_ty,
                    dst,
                    dst_ty,
                },
            ]),

            /* 'movsx' requires a GP reg dst.  */
            AsmInstr::Movsx {
                src,
                src_ty,
                dst,
                dst_ty,
            } if is_mem(&dst) => Ok(vec![
                AsmInstr::Movsx {
                    src,
                    src_ty,
                    dst: AsmOp::Reg(AsmReg::R9),
                    dst_ty,
                },
                AsmInstr::Mov {
                    src: AsmOp::Reg(AsmReg::R9),
                    dst,
                    asm_ty: dst_ty,
                },
            ]),

            /* Int binops cannot have large 64-bit immediates.  */
            AsmInstr::Bin {
                kind,
                lhs: AsmOp::Imm(n),
                rhs,
                asm_ty: AsmType::Quad,
            } if is_large(n) => Ok(vec![
                AsmInstr::Mov {
                    src: AsmOp::Imm(n),
                    dst: AsmOp::Reg(AsmReg::R9),
                    asm_ty: AsmType::Quad,
                },
                AsmInstr::Bin {
                    kind,
                    lhs: AsmOp::Reg(AsmReg::R9),
                    rhs,
                    asm_ty: AsmType::Quad,
                },
            ]),

            /* Floating-point binops require an SSE register as dst.  */
            AsmInstr::Bin {
                kind,
                lhs,
                rhs,
                asm_ty,
            } if matches!(asm_ty, AsmType::Float | AsmType::Double) && is_mem(&rhs) => Ok(vec![
                AsmInstr::Mov {
                    src: rhs.clone(),
                    dst: AsmOp::Reg(AsmReg::Xmm15),
                    asm_ty,
                },
                AsmInstr::Bin {
                    kind,
                    lhs: lhs,
                    rhs: AsmOp::Reg(AsmReg::Xmm15),
                    asm_ty,
                },
                AsmInstr::Mov {
                    src: AsmOp::Reg(AsmReg::Xmm15),
                    dst: rhs,
                    asm_ty,
                },
            ]),

            /* Int binops cannot have both operands in mem.  */
            AsmInstr::Bin {
                kind,
                lhs,
                rhs,
                asm_ty,
            } if is_mem(&lhs) && is_mem(&rhs) => Ok(vec![
                AsmInstr::Mov {
                    src: lhs,
                    dst: AsmOp::Reg(AsmReg::R9),
                    asm_ty,
                },
                AsmInstr::Bin {
                    kind,
                    lhs: AsmOp::Reg(AsmReg::R9),
                    rhs: rhs,
                    asm_ty,
                },
            ]),

            /* Floating-point comparisons cannot have a mem dst.  */
            AsmInstr::Cmp { asm_ty, src, dst }
                if matches!(asm_ty, AsmType::Float | AsmType::Double) && is_mem(&dst) =>
            {
                Ok(vec![
                    AsmInstr::Mov {
                        asm_ty,
                        src: dst,
                        dst: AsmOp::Reg(AsmReg::Xmm15),
                    },
                    AsmInstr::Cmp {
                        asm_ty,
                        src,
                        dst: AsmOp::Reg(AsmReg::Xmm15),
                    },
                ])
            }

            /* Not both operands of 'cmp' can be in mem.  */
            AsmInstr::Cmp { asm_ty, src, dst } if is_mem(&src) && is_mem(&dst) => Ok(vec![
                AsmInstr::Mov {
                    asm_ty,
                    src,
                    dst: AsmOp::Reg(AsmReg::R9),
                },
                AsmInstr::Cmp {
                    asm_ty,
                    src: AsmOp::Reg(AsmReg::R9),
                    dst,
                },
            ]),

            /* 'cmpq' cannot compare two immediates; a large immediate also cannot be encoded directly.  */
            AsmInstr::Cmp {
                asm_ty: AsmType::Quad,
                src: AsmOp::Imm(imm1),
                dst: AsmOp::Imm(imm2),
            } if is_large(imm1) => Ok(vec![
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
            ]),

            /* 'cmpq' src cannot have large src immediate.  */
            AsmInstr::Cmp {
                asm_ty: AsmType::Quad,
                src: AsmOp::Imm(imm1),
                dst,
            } if is_large(imm1) => Ok(vec![
                AsmInstr::Mov {
                    asm_ty: AsmType::Quad,
                    src: AsmOp::Imm(imm1),
                    dst: AsmOp::Reg(AsmReg::R9),
                },
                AsmInstr::Cmp {
                    asm_ty: AsmType::Quad,
                    src: AsmOp::Reg(AsmReg::R9),
                    dst,
                },
            ]),

            /* The dst operand of 'cmp' cannot be an immediate.  */
            AsmInstr::Cmp {
                asm_ty,
                src,
                dst: AsmOp::Imm(imm),
            } => Ok(vec![
                AsmInstr::Mov {
                    asm_ty,
                    src: AsmOp::Imm(imm),
                    dst: AsmOp::Reg(AsmReg::R9),
                },
                AsmInstr::Cmp {
                    asm_ty,
                    src,
                    dst: AsmOp::Reg(AsmReg::R9),
                },
            ]),

            /* 'cvtsi2sd' cannot have an immediate src.  */
            AsmInstr::Cvtsi2sd {
                src: AsmOp::Imm(n),
                dst,
                asm_ty,
            } => Ok(vec![
                AsmInstr::Mov {
                    src: AsmOp::Imm(n),
                    dst: AsmOp::Reg(AsmReg::R9),
                    asm_ty,
                },
                AsmInstr::Cvtsi2sd {
                    src: AsmOp::Reg(AsmReg::R9),
                    dst,
                    asm_ty,
                },
            ]),

            /* 'cvtsi2sd' cannot have a dst operand in mem.  */
            AsmInstr::Cvtsi2sd { src, dst, asm_ty } if is_mem(&dst) => Ok(vec![
                AsmInstr::Cvtsi2sd {
                    src,
                    dst: AsmOp::Reg(AsmReg::Xmm15),
                    asm_ty,
                },
                AsmInstr::Mov {
                    src: AsmOp::Reg(AsmReg::Xmm15),
                    dst,
                    asm_ty: AsmType::Double,
                },
            ]),

            /* 'cvtsi2ss' cannot have an immediate src. */
            AsmInstr::Cvtsi2ss {
                src: AsmOp::Imm(n),
                dst,
                asm_ty,
            } => Ok(vec![
                AsmInstr::Mov {
                    src: AsmOp::Imm(n),
                    dst: AsmOp::Reg(AsmReg::R9),
                    asm_ty,
                },
                AsmInstr::Cvtsi2ss {
                    src: AsmOp::Reg(AsmReg::R9),
                    dst,
                    asm_ty,
                },
            ]),

            /* 'cvtsi2ss' cannot have a dst operand in mem.  */
            AsmInstr::Cvtsi2ss { src, dst, asm_ty } if is_mem(&dst) => Ok(vec![
                AsmInstr::Cvtsi2ss {
                    src,
                    dst: AsmOp::Reg(AsmReg::Xmm15),
                    asm_ty,
                },
                AsmInstr::Mov {
                    src: AsmOp::Reg(AsmReg::Xmm15),
                    dst,
                    asm_ty: AsmType::Float,
                },
            ]),

            /* 'cvttsd2si' cannot have a dst operand in mem.  */
            AsmInstr::Cvttsd2si { src, dst, asm_ty } if is_mem(&dst) => Ok(vec![
                AsmInstr::Cvttsd2si {
                    src,
                    dst: AsmOp::Reg(AsmReg::R9),
                    asm_ty,
                },
                AsmInstr::Mov {
                    src: AsmOp::Reg(AsmReg::R9),
                    dst,
                    asm_ty,
                },
            ]),

            /* 'cvttss2si' cannot have a dst operand in mem.  */
            AsmInstr::Cvttss2si { src, dst, asm_ty } if is_mem(&dst) => Ok(vec![
                AsmInstr::Cvttss2si {
                    src,
                    dst: AsmOp::Reg(AsmReg::R9),
                    asm_ty,
                },
                AsmInstr::Mov {
                    src: AsmOp::Reg(AsmReg::R9),
                    dst,
                    asm_ty,
                },
            ]),

            /* 'cvtss2sd' cannot have a dst operand in mem.  */
            AsmInstr::Cvtss2sd { src, dst } if is_mem(&dst) => Ok(vec![
                AsmInstr::Cvtss2sd {
                    src,
                    dst: AsmOp::Reg(AsmReg::Xmm15),
                },
                AsmInstr::Mov {
                    src: AsmOp::Reg(AsmReg::Xmm15),
                    dst,
                    asm_ty: AsmType::Double,
                },
            ]),

            /* 'cvtsd2ss' cannot have a dst operand in mem.  */
            AsmInstr::Cvtsd2ss { src, dst } if is_mem(&dst) => Ok(vec![
                AsmInstr::Cvtsd2ss {
                    src,
                    dst: AsmOp::Reg(AsmReg::Xmm15),
                },
                AsmInstr::Mov {
                    src: AsmOp::Reg(AsmReg::Xmm15),
                    dst,
                    asm_ty: AsmType::Float,
                },
            ]),

            instr => Err(instr),
        }
    }
}

fn is_large(n: i64) -> bool {
    n > i32::MAX as i64 || n < i32::MIN as i64
}

fn is_mem(op: &AsmOp) -> bool {
    matches!(op, AsmOp::Mem(_, _) | AsmOp::Data(_, _))
}
