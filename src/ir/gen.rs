use std::{any::TypeId, collections::HashMap, sync::atomic::AtomicU64};

use crate::{
    bytecode::r#gen::BytecodeFunc,
    parser::ast::{
        BinaryExpressionKind, BlockItem, Declaration, Expression, Literal, Param, Program,
        Statement, Type,
    },
};

pub trait Codegen<T, U> {
    fn codegen(&self, ctx: &mut U) -> Result<T, Box<dyn std::error::Error>>;
}

#[derive(Debug)]
pub struct CodegenContext {
    pub locals: HashMap<String, usize>,
    pub funcs: HashMap<String, usize>,
}

impl CodegenContext {
    pub fn new() -> Self {
        Self {
            locals: HashMap::new(),
            funcs: HashMap::new(),
        }
    }
}

impl Ctx for CodegenContext {
    fn get<T: 'static>(&self, name: &str) -> Option<usize> {
        match TypeId::of::<T>() {
            id if id == TypeId::of::<String>() => self.locals.get(name).copied(),
            id if id == TypeId::of::<BytecodeFunc>() => self.funcs.get(name).copied(),

            _ => unreachable!(),
        }
    }

    fn register<T: 'static>(&mut self, name: &str) -> usize {
        let structure = match TypeId::of::<T>() {
            id if id == TypeId::of::<String>() => &mut self.locals,
            id if id == TypeId::of::<BytecodeFunc>() => &mut self.funcs,
            _ => unreachable!(),
        };

        let idx = structure.len();

        structure.insert(name.to_owned(), idx);

        idx
    }
}

pub trait Ctx {
    fn get<T: 'static>(&self, name: &str) -> Option<usize>;
    fn register<T: 'static>(&mut self, name: &str) -> usize;
}

#[derive(Debug, Clone)]
pub enum IRVal {
    Const(Literal),
    Var(String),
}

#[derive(Debug, Clone)]
pub enum IRInstr {
    Cpy {
        src: IRVal,
        dst: IRVal,
    },
    Ret {
        val: Option<IRVal>,
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
    Jmp {
        target: String,
    },
    Jz {
        cond: IRVal,
        target: String,
    },
    Label(String),
    Call {
        target: String,
        args: Vec<IRVal>,
        dst: IRVal,
    },
}

#[derive(Debug)]
pub struct IRProg {
    pub funcs: Vec<IRFunc>,
}

#[derive(Debug, Clone)]
pub struct IRFunc {
    pub name: String,
    pub params: Vec<Param>,
    pub body: Vec<IRInstr>,
    pub ret_ty: Option<Type>,
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
                params: params.to_vec(),
                body: vec![],
                ret_ty: ret.clone(),
            };

            if let Statement::Block { body } = body {
                for stmt in body {
                    irfy_stmt(&mut f.body, stmt);
                }
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

        Statement::Ret { value, ret_ty: _ } => {
            if value.is_some() {
                let r = irfy_expr_and_cvt(instrs, value.as_ref().unwrap());

                instrs.push(IRInstr::Ret { val: Some(r) });
            } else {
                instrs.push(IRInstr::Ret { val: None });
            }
        }

        Statement::Print { value } => {
            let r = irfy_expr_and_cvt(instrs, value);

            instrs.push(IRInstr::Print { val: r });
        }

        Statement::If {
            cond,
            then_block,
            else_block,
        } => {
            let r = irfy_expr_and_cvt(instrs, cond);

            let tmp = mktmp();

            if else_block.is_none() {
                instrs.push(IRInstr::Jz {
                    cond: r,
                    target: format!("End.{}", tmp),
                });

                irfy_stmt(instrs, then_block);

                instrs.push(IRInstr::Label(format!("End.{}", tmp)));
            } else {
                instrs.push(IRInstr::Jz {
                    cond: r,
                    target: format!("Else.{}", tmp),
                });
                irfy_stmt(instrs, then_block);

                instrs.push(IRInstr::Jmp {
                    target: format!("End.{}", tmp),
                });

                instrs.push(IRInstr::Label(format!("Else.{}", tmp)));

                irfy_stmt(instrs, else_block.as_ref().unwrap());

                instrs.push(IRInstr::Label(format!("End.{}", tmp)));
            }
        }

        Statement::Block { body } => {
            for stmt in body {
                irfy_stmt(instrs, stmt);
            }
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
        Expression::Literal { val, ty, span } => ExpRes::Plain(IRVal::Const(*val)),
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

        Expression::Call {
            target,
            args,
            ty,
            span,
        } => {
            let ir_args = args
                .iter()
                .map(|arg| irfy_expr_and_cvt(instrs, arg))
                .collect();

            let dst = mkirvar();

            instrs.push(IRInstr::Call {
                target: target.to_owned(),
                args: ir_args,
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

pub fn mktmp() -> u64 {
    static NEXT_TMP: AtomicU64 = AtomicU64::new(0);

    NEXT_TMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
