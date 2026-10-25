use std::{any::TypeId, collections::HashMap, sync::atomic::AtomicU64};

use crate::{
    bytecode::r#gen::BytecodeFunc,
    parser::ast::{
        BinaryExpressionKind, BlockItem, Declaration, Expression, Literal, Param, Program,
        Statement, Type, UnaryExpressionKind,
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

impl Default for CodegenContext {
    fn default() -> Self {
        Self::new()
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
        operand_ty: Type,
    },
    Unary {
        kind: UnaryExpressionKind,
        src: IRVal,
        dst: IRVal,
        operand_ty: Type,
    },
    Print {
        val: IRVal,
        ty: Type,
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
        arg_tys: Vec<Type>,
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
        BlockItem::Stmt(_stmt) => unimplemented!(),
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
                ret_ty: *ret,
            };

            if let Statement::Block { body } = body {
                for stmt in body {
                    irfy_stmt(&mut f.body, stmt);
                }
            }

            if ret.is_none() {
                f.body.push(IRInstr::Ret { val: None });
            }

            ir_prog.funcs.push(f);
        }
    }
}

fn irfy_stmt(instrs: &mut Vec<IRInstr>, stmt: &Statement) {
    match stmt {
        Statement::Let { name, init, ty: _ } => {
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
            let ty = value.ty().unwrap();
            let r = irfy_expr_and_cvt(instrs, value);

            instrs.push(IRInstr::Print { val: r, ty });
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

        Statement::While { cond, body, label } => {
            let label = label.as_ref().unwrap();

            let break_label = format!("{label}.break");
            let continue_label = format!("{label}.continue");

            instrs.push(IRInstr::Label(continue_label.clone()));

            /* eval the cond at the start of each iteration  */
            let cond = irfy_expr_and_cvt(instrs, cond);

            /* exit the loop if the cond is false  */
            instrs.push(IRInstr::Jz {
                cond,
                target: break_label.clone(),
            });

            /* exec the loop body  */
            irfy_stmt(instrs, body);

            /* start the next iteration  */
            instrs.push(IRInstr::Jmp {
                target: continue_label,
            });

            /* resume execution here after exiting the loop  */
            instrs.push(IRInstr::Label(break_label));
        }

        Statement::DoWhile { cond, body, label } => {
            let label = label.as_ref().unwrap();

            let start_label = format!("{label}.start");
            let break_label = format!("{label}.break");
            let continue_label = format!("{label}.continue");

            instrs.push(IRInstr::Label(start_label.clone()));

            /* exec the loop body at least once */
            irfy_stmt(instrs, body);

            /* continue stmts jmp here to eval the cond  */
            instrs.push(IRInstr::Label(continue_label.clone()));

            /* eval the cond  */
            let cond = irfy_expr_and_cvt(instrs, cond);

            /* exit the loop if the cond is false  */
            instrs.push(IRInstr::Jz {
                cond,
                target: break_label.clone(),
            });

            /* start the next iteration  */
            instrs.push(IRInstr::Jmp {
                target: continue_label,
            });

            /* resume execution here after exiting the loop  */
            instrs.push(IRInstr::Label(break_label));
        }

        Statement::For {
            init,
            cond,
            advancement,
            body,
            label,
        } => {
            let label = label.as_ref().unwrap();

            let start_label = format!("{label}.start");
            let break_label = format!("{label}.break");
            let continue_label = format!("{label}.continue");

            /* exec the init once  */
            if let Some(init) = init {
                irfy_stmt(instrs, init);
            }

            /* eval the cond at the start of each iteration  */
            instrs.push(IRInstr::Label(start_label.clone()));

            if let Some(cond) = cond {
                let cond = irfy_expr_and_cvt(instrs, cond);

                instrs.push(IRInstr::Jz {
                    cond,
                    target: break_label.clone(),
                });
            }

            /* exec the loop body  */
            irfy_stmt(instrs, body);

            /* continue stmts jmp here, before the advancement  */
            instrs.push(IRInstr::Label(continue_label));

            /* exec the advancement  */
            if let Some(advancement) = advancement {
                irfy_expr(instrs, advancement);
            }

            /* re-eval the cond  */
            instrs.push(IRInstr::Jmp {
                target: start_label,
            });

            /* resume execution here  */
            instrs.push(IRInstr::Label(break_label));
        }

        Statement::Loop { body, label } => {
            let label = label.as_ref().unwrap();

            let break_label = format!("{label}.break");
            let continue_label = format!("{label}.continue");

            /* start the loop body  */
            instrs.push(IRInstr::Label(continue_label.clone()));

            /* exec the body  */
            irfy_stmt(instrs, body);

            /* repeat indefinitely  */
            instrs.push(IRInstr::Jmp {
                target: continue_label,
            });

            /* resume execution here  */
            instrs.push(IRInstr::Label(break_label));
        }

        Statement::Break { label } => {
            let label = label.as_ref().unwrap();
            instrs.push(IRInstr::Jmp {
                target: format!("{label}.break"),
            });
        }

        Statement::Continue { label } => {
            let label = label.as_ref().unwrap();
            instrs.push(IRInstr::Jmp {
                target: format!("{label}.continue"),
            });
        }

        Statement::Goto { label } => {
            instrs.push(IRInstr::Jmp {
                target: format!("goto.{label}"),
            });
        }

        Statement::Labeled { name, stmt } => {
            instrs.push(IRInstr::Label(format!("goto.{name}")));
            irfy_stmt(instrs, stmt);
        }

        Statement::Expr { expr } => {
            irfy_expr(instrs, expr);
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
        Expression::Literal { val, ty, span: _ } => {
            let ty = ty.unwrap();
            let literal = Literal::try_from((val, ty)).unwrap();

            ExpRes::Plain(IRVal::Const(literal))
        }
        Expression::Variable {
            name,
            ty: _,
            span: _,
        } => ExpRes::Plain(IRVal::Var(name.to_owned())),
        Expression::Unary { kind, expr, ty, .. } => {
            let operand_ty = ty.unwrap();

            let src = irfy_expr_and_cvt(instrs, expr);

            let dst = mkirvar();

            instrs.push(IRInstr::Unary {
                kind: *kind,
                src,
                dst: dst.clone(),
                operand_ty,
            });

            ExpRes::Plain(dst)
        }
        Expression::Binary {
            kind,
            lhs,
            rhs,
            ty: _,
            span: _,
        } => {
            let operand_ty = lhs.ty().unwrap();

            let lhs = irfy_expr_and_cvt(instrs, lhs);
            let rhs = irfy_expr_and_cvt(instrs, rhs);

            let dst = mkirvar();

            instrs.push(IRInstr::Bin {
                kind: *kind,
                lhs,
                rhs,
                dst: dst.clone(),
                operand_ty,
            });

            ExpRes::Plain(dst)
        }

        Expression::Call {
            target,
            args,
            ty: _,
            span: _,
        } => {
            let arg_tys = args.iter().map(|arg| arg.ty().unwrap()).collect();

            let ir_args = args
                .iter()
                .map(|arg| irfy_expr_and_cvt(instrs, arg))
                .collect();

            let dst = mkirvar();

            instrs.push(IRInstr::Call {
                target: target.to_owned(),
                args: ir_args,
                arg_tys,
                dst: dst.clone(),
            });

            ExpRes::Plain(dst)
        }

        Expression::Assign { lhs, rhs, .. } => {
            /* Keep lhs as lvalue.  */
            let lhs = irfy_expr(instrs, lhs);

            /* Force lvalue-to-rvalue conversion for rhs.  */
            let rhs = irfy_expr_and_cvt(instrs, rhs);

            /* If lhs is a plain operand, just emit Cpy.  */

            match lhs {
                ExpRes::Plain(op) => {
                    instrs.push(IRInstr::Cpy {
                        src: rhs.clone(),
                        dst: op,
                    });
                }
            }

            /* Assignments evaluate to their rhs val.  */
            ExpRes::Plain(rhs)
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
