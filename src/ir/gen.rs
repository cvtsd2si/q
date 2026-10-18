use std::sync::atomic::AtomicU64;

use crate::parser::ast::{
    BinaryExpressionKind, BlockItem, Declaration, Expression, Program, Statement,
};

pub trait Codegen<T> {
    fn codegen(&self) -> T;
}

#[derive(Debug, Clone)]
pub enum IRVal {
    Const(i64),
    Var(String),
}

#[derive(Debug, Clone)]
pub enum IRInstr {
    Cpy {
        src: IRVal,
        dst: IRVal,
    },
    Ret {
        val: IRVal,
    },
    Bin {
        kind: BinaryExpressionKind,
        lhs: IRVal,
        rhs: IRVal,
        dst: IRVal,
    },
    Print {
        val: IRVal,
    },
}

#[derive(Debug)]
pub struct IRProg {
    pub funcs: Vec<IRFunc>,
}

#[derive(Debug, Clone)]
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

        Statement::Print { value } => {
            let r = irfy_expr_and_cvt(instrs, value);

            instrs.push(IRInstr::Print { val: r });
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
            let lhs = irfy_expr_and_cvt(instrs, lhs);
            let rhs = irfy_expr_and_cvt(instrs, rhs);

            let dst = mkirvar();

            instrs.push(IRInstr::Bin {
                kind: *kind,
                lhs,
                rhs,
                dst: dst.clone(),
            });

            ExpRes::Plain(dst)
        }
    }
}

fn mkirvar() -> IRVal {
    let tmp = mktmp();

    IRVal::Var(format!("tmp.{}", tmp))
}

fn mktmp() -> u64 {
    static NEXT_TMP: AtomicU64 = AtomicU64::new(0);

    NEXT_TMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
