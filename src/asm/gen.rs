use crate::{
    ir::r#gen::{Codegen, IRFunc, IRInstr, IRProg, IRVal},
    parser::ast::{BinaryExpressionKind, Literal, Type},
    runtime::value::ValueTag,
};

#[derive(Debug)]
pub struct AsmProg {
    pub funcs: Vec<AsmFunc>,
}

#[derive(Debug)]
pub struct AsmFunc {
    pub name: String,
    pub body: Vec<AsmInstr>,
    pub space: usize,
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
    Stack(isize),
}

#[derive(Debug, Clone, Copy)]
pub enum AsmReg {
    Bp,
    Sp,
    Ax,
    R8,
    R9,
    R10,
    R11,
    R12,
    R13,
    Cx,
    Dx,
    Di,
    Si,
}

impl Codegen<AsmProg> for IRProg {
    fn codegen(&self) -> AsmProg {
        AsmProg {
            funcs: self.funcs.iter().map(|f| f.codegen()).collect(),
        }
    }
}

impl Codegen<AsmFunc> for IRFunc {
    fn codegen(&self) -> AsmFunc {
        let mut body = vec![];

        insert_prologue(&mut body);

        for instr in &self.body {
            body.extend(instr.codegen_with_ret_ty(self.ret_ty));
        }

        AsmFunc {
            name: self.name.to_owned(),
            body,
            space: 0,
        }
    }
}

impl IRInstr {
    fn codegen_with_ret_ty(&self, ret_ty: Type) -> Vec<AsmInstr> {
        match self {
            IRInstr::Cpy { src, dst } => vec![AsmInstr::Mov {
                src: src.codegen(),
                dst: dst.codegen(),
            }],

            IRInstr::Ret { val } => {
                let mut instrs = vec![
                    AsmInstr::Mov {
                        src: val.codegen(),
                        dst: AsmOp::Reg(AsmReg::Ax),
                    },
                    AsmInstr::Mov {
                        src: AsmOp::Imm(ValueTag::from(ret_ty) as i64),
                        dst: AsmOp::Reg(AsmReg::Dx),
                    },
                ];

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

                let instrs = vec![
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

            IRInstr::Print { val: _ } => todo!(),
        }
    }
}

impl Codegen<AsmOp> for IRVal {
    fn codegen(&self) -> AsmOp {
        match self {
            IRVal::Const(konst) => match konst {
                Literal::UInt8(uint8) => AsmOp::Imm(*uint8 as i64),
                Literal::Int8(int8) => AsmOp::Imm(*int8 as i64),
                Literal::UInt16(uint16) => AsmOp::Imm(*uint16 as i64),
                Literal::Int16(int16) => AsmOp::Imm(*int16 as i64),
                Literal::UInt32(uint32) => AsmOp::Imm(*uint32 as i64),
                Literal::Int32(int32) => AsmOp::Imm(*int32 as i64),
                Literal::UInt64(uint64) => AsmOp::Imm(*uint64 as i64),
                Literal::Int64(int64) => AsmOp::Imm(*int64),
                Literal::Bool(boolean) => AsmOp::Imm(if *boolean { 1 } else { 0 }),
                _ => unimplemented!(),
            },
            IRVal::Var(var) => AsmOp::Pseudo(var.to_owned()),
        }
    }
}

pub trait AsmSink {
    fn sink(&mut self, instr: AsmInstr);
}

impl AsmSink for Vec<AsmInstr> {
    fn sink(&mut self, instr: AsmInstr) {
        self.push(instr);
    }
}

impl AsmSink for String {
    fn sink(&mut self, instr: AsmInstr) {
        self.push_str(&instr.to_string());
    }
}

pub fn insert_prologue<T>(body: &mut T)
where
    T: AsmSink,
{
    body.sink(AsmInstr::Push(AsmOp::Reg(AsmReg::Bp)));
    body.sink(AsmInstr::Mov {
        src: AsmOp::Reg(AsmReg::Sp),
        dst: AsmOp::Reg(AsmReg::Bp),
    });
    body.sink(AsmInstr::Bin {
        kind: AsmInstrBinKind::Sub,
        lhs: AsmOp::Imm(0),
        rhs: AsmOp::Reg(AsmReg::Sp),
    });
}

pub fn insert_epilogue<T>(body: &mut T)
where
    T: AsmSink,
{
    body.sink(AsmInstr::Mov {
        src: AsmOp::Reg(AsmReg::Bp),
        dst: AsmOp::Reg(AsmReg::Sp),
    });
    body.sink(AsmInstr::Pop(AsmOp::Reg(AsmReg::Bp)));
}
