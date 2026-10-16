use crate::ir::r#gen::{IRInstr, IRProg, IRVal};

#[derive(Debug)]
pub struct AsmProg {
    instrs: Vec<AsmInstr>,
}

#[derive(Debug)]
enum AsmInstr {
    Mov { src: AsmOp, dst: AsmOp },
    Ret,
}

#[derive(Debug)]
enum AsmOp {
    Pseudo(String),
    Reg(AsmReg),
    Imm(i64),
}

#[derive(Debug)]
enum AsmReg {
    Bp,
    Sp,
    Ax,
}

pub fn codegen_ir_prog(ir_prog: &IRProg) -> AsmProg {
    let mut asm_prog = AsmProg { instrs: vec![] };

    for ir_instr in &ir_prog.instrs {
        codegen_ir_instr(&mut asm_prog.instrs, ir_instr);
    }

    asm_prog
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
