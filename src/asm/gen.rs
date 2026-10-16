use crate::ir::r#gen::{IRFunc, IRInstr, IRProg, IRVal};

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
    Mov { src: AsmOp, dst: AsmOp },
    Ret,
    Push(AsmOp),
    Pop(AsmOp),
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

pub fn codegen(ir_prog: &IRProg) -> AsmProg {
    let mut asm_prog = AsmProg { funcs: vec![] };

    for ir_func in &ir_prog.funcs {
        codegen_ir_func(&mut asm_prog.funcs, ir_func);
    }

    asm_prog
}

fn codegen_ir_func(funcs: &mut Vec<AsmFunc>, ir_func: &IRFunc) {
    let mut f = AsmFunc {
        name: ir_func.name.to_owned(),
        body: vec![],
    };

    insert_prologue(&mut f.body);

    for ir_instr in &ir_func.body {
        codegen_ir_instr(&mut f.body, ir_instr);
    }

    funcs.push(f);
}

fn codegen_ir_instr(instrs: &mut Vec<AsmInstr>, ir_instr: &IRInstr) {
    match ir_instr {
        IRInstr::Cpy { src, dst } => {
            let src = codegen_ir_val(src);
            let dst = codegen_ir_val(dst);

            instrs.push(AsmInstr::Mov { src, dst });
        }

        IRInstr::Ret { val } => {
            let v = codegen_ir_val(val);

            instrs.push(AsmInstr::Mov {
                src: v,
                dst: AsmOp::Reg(AsmReg::Ax),
            });

            insert_epilogue(instrs);

            instrs.push(AsmInstr::Ret);
        }
    }
}

fn codegen_ir_val(ir_val: &IRVal) -> AsmOp {
    match ir_val {
        IRVal::Const(konst) => AsmOp::Imm(*konst),
        IRVal::Var(var) => AsmOp::Pseudo(var.to_owned()),
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
