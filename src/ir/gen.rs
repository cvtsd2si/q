use crate::parser::ast::{BlockItem, Declaration, Expression, Program, Statement};

#[derive(Debug)]
pub enum IRVal {
    Const(i64),
    Var(String),
}

#[derive(Debug)]
pub enum IRInstr {
    Cpy { src: IRVal, dst: IRVal },
    Ret { val: IRVal },
}

#[derive(Debug)]
pub struct IRProg {
    pub funcs: Vec<IRFunc>,
}

#[derive(Debug)]
pub struct IRFunc {
    pub name: String,
    pub body: Vec<IRInstr>,
}

pub fn irfy(ast: &Program) -> IRProg {
    let mut ir_prog = IRProg { funcs: vec![] };

    for block_item in &ast.block_items {
        irfy_block_item(&mut ir_prog, block_item);
    }

    ir_prog
}

fn irfy_block_item(ir_prog: &mut IRProg, block_item: &BlockItem) {
    match block_item {
        BlockItem::Decl(decl) => irfy_decl(ir_prog, decl),
        BlockItem::Stmt(stmt) => unimplemented!(),
    }
}

fn irfy_decl(ir_prog: &mut IRProg, decl: &Declaration) {
    match decl {
        Declaration::Fn {
            name,
            ret,
            params,
            body,
        } => {
            let mut f = IRFunc {
                name: name.to_owned(),
                body: vec![],
            };

            for stmt in body {
                irfy_stmt(&mut f.body, stmt);
            }

            ir_prog.funcs.push(f);
        }
    }
}

fn irfy_stmt(instrs: &mut Vec<IRInstr>, stmt: &Statement) {
    match stmt {
        Statement::Let { name, init, ty } => {
            let r = irfy_expr_and_cvt(instrs, init);
            let dst = IRVal::Var(name.to_string());

            instrs.push(IRInstr::Cpy { src: r, dst });
        }

        Statement::Ret { value } => {
            let r = irfy_expr_and_cvt(instrs, value);

            instrs.push(IRInstr::Ret { val: r });
        }
    }
}

enum ExpRes {
    Plain(IRVal),
}

fn irfy_expr_and_cvt(instrs: &mut Vec<IRInstr>, expr: &Expression) -> IRVal {
    let r = irfy_expr(instrs, expr);

    match r {
        ExpRes::Plain(val) => val,
    }
}

fn irfy_expr(instrs: &mut Vec<IRInstr>, expr: &Expression) -> ExpRes {
    match expr {
        Expression::Literal { val, span } => ExpRes::Plain(IRVal::Const(*val)),
        Expression::Variable { name, ty, span } => ExpRes::Plain(IRVal::Var(name.to_owned())),
        Expression::Binary {
            kind,
            lhs,
            rhs,
            ty,
            span,
        } => {
            todo!()
        }
    }
}
