use crate::{
    ir::r#gen::{IRFunc, IRInstr, IRProg, IRVal},
    parser::ast::BinaryExpressionKind,
};

#[derive(Debug)]
pub struct AsmProg {
    pub funcs: Vec<AsmFunc>,
}

#[derive(Debug)]
pub struct AsmFunc {
    pub name: String,
    pub body: Vec<AsmInstr>,
}

#[derive(Debug, Clone)]
pub enum AsmInstr {
    Mov {
        src: AsmOp,
        dst: AsmOp,
    },
    Ret,
    Push(AsmOp),
    Pop(AsmOp),
    Bin {
        kind: AsmInstrBinKind,
        lhs: AsmOp,
        rhs: AsmOp,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum AsmInstrBinKind {
    Add,
    Sub,
}

#[derive(Debug, Clone)]
pub enum AsmOp {
    Pseudo(String),
    Reg(AsmReg),
    Imm(i64),
}

#[derive(Debug, Clone, Copy)]
pub enum AsmReg {
    Bp,
    Sp,
    Ax,
    R9,
}

pub trait Codegen {
    type Output;

    fn codegen(&self) -> Self::Output;
}

impl Codegen for IRProg {
    type Output = AsmProg;

    fn codegen(&self) -> Self::Output {
        AsmProg {
            funcs: self.funcs.iter().map(|f| f.codegen()).collect(),
        }
    }
}

impl Codegen for IRFunc {
    type Output = AsmFunc;

    fn codegen(&self) -> Self::Output {
        let mut body = vec![];

        insert_prologue(&mut body);

        for instr in &self.body {
            body.extend(instr.codegen());
        }

        AsmFunc {
            name: self.name.to_owned(),
            body,
        }
    }
}

impl Codegen for IRInstr {
    type Output = Vec<AsmInstr>;

    fn codegen(&self) -> Self::Output {
        match self {
            IRInstr::Cpy { src, dst } => vec![AsmInstr::Mov {
                src: src.codegen(),
                dst: dst.codegen(),
            }],

            IRInstr::Ret { val } => {
                let mut instrs = vec![AsmInstr::Mov {
                    src: val.codegen(),
                    dst: AsmOp::Reg(AsmReg::Ax),
                }];

                insert_epilogue(&mut instrs);

                instrs.push(AsmInstr::Ret);

                instrs
            }

            IRInstr::Bin {
                kind,
                lhs,
                rhs,
                dst,
            } => {
                let kind = match kind {
                    BinaryExpressionKind::Add => AsmInstrBinKind::Add,
                    BinaryExpressionKind::Sub => AsmInstrBinKind::Sub,
                    _ => unimplemented!(),
                };

                let mut instrs = vec![
                    AsmInstr::Mov {
                        src: lhs.codegen(),
                        dst: dst.codegen(),
                    },
                    AsmInstr::Bin {
                        kind,
                        lhs: rhs.codegen(),
                        rhs: dst.codegen(),
                    },
                ];

                instrs
            }
        }
    }
}

impl Codegen for IRVal {
    type Output = AsmOp;

    fn codegen(&self) -> Self::Output {
        match self {
            IRVal::Const(konst) => AsmOp::Imm(*konst),
            IRVal::Var(var) => AsmOp::Pseudo(var.to_owned()),
        }
    }
}

fn insert_prologue(body: &mut Vec<AsmInstr>) {
    body.push(AsmInstr::Push(AsmOp::Reg(AsmReg::Bp)));
    body.push(AsmInstr::Mov {
        src: AsmOp::Reg(AsmReg::Sp),
        dst: AsmOp::Reg(AsmReg::Bp),
    });
}

fn insert_epilogue(body: &mut Vec<AsmInstr>) {
    body.push(AsmInstr::Mov {
        src: AsmOp::Reg(AsmReg::Bp),
        dst: AsmOp::Reg(AsmReg::Sp),
    });
    body.push(AsmInstr::Pop(AsmOp::Reg(AsmReg::Bp)));
}
