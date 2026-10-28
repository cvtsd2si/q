use std::collections::HashMap;

use crate::{
    ir::r#gen::mktmp,
    parser::ast::{BlockItem, Declaration, Expression, Param, Program, Statement, Type},
};

pub struct Resolver {
    var_map: HashMap<String, Var>,
}

impl Resolver {
    pub fn new() -> Self {
        Self {
            var_map: HashMap::new(),
        }
    }
}

impl Default for Resolver {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct Var {
    pub name: String,
}

pub trait Resolve {
    type Output;
    fn resolve(self, resolver: &mut Resolver) -> Result<Self::Output, ResolverError>;
}

impl Resolve for Program {
    type Output = Program;

    fn resolve(self, resolver: &mut Resolver) -> Result<Self::Output, ResolverError> {
        let resolved = self
            .block_items
            .into_iter()
            .map(|block_item| block_item.resolve(resolver))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Program {
            block_items: resolved,
        })
    }
}

impl Resolve for BlockItem {
    type Output = BlockItem;

    fn resolve(self, resolver: &mut Resolver) -> Result<Self::Output, ResolverError> {
        match self {
            BlockItem::Decl(decl) => {
                let resolved = decl.resolve(resolver)?;
                Ok(BlockItem::Decl(resolved))
            }

            BlockItem::Stmt(stmt) => {
                let resolved = stmt.resolve(resolver)?;
                Ok(BlockItem::Stmt(resolved))
            }
        }
    }
}

impl Resolve for Declaration {
    type Output = Declaration;

    fn resolve(self, resolver: &mut Resolver) -> Result<Self::Output, ResolverError> {
        match self {
            Declaration::Fn {
                name,
                ret,
                params,
                body,
            } => {
                let resolved = resolve_fn(resolver, name, ret, params, body)?;
                Ok(resolved)
            }
        }
    }
}

impl Resolve for Statement {
    type Output = Statement;

    fn resolve(self, resolver: &mut Resolver) -> Result<Self::Output, ResolverError> {
        match self {
            Statement::Let { name, init, ty } => resolve_let(resolver, name, init, ty),
            Statement::Ret { value, ret_ty } => resolve_ret(resolver, value, ret_ty),
            Statement::Print { value } => resolve_print(resolver, value),
            Statement::If {
                cond,
                then_block,
                else_block,
            } => resolve_if(resolver, cond, then_block, else_block),
            Statement::Block { body } => Ok(Statement::Block {
                body: resolve_block(resolver, body)?,
            }),
            Statement::While { cond, body, label } => resolve_while(resolver, cond, body, label),
            Statement::DoWhile { cond, body, label } => {
                resolve_do_while(resolver, cond, body, label)
            }
            Statement::For {
                init,
                cond,
                advancement,
                body,
                label,
            } => resolve_for(resolver, init, cond, advancement, body, label),
            Statement::Loop { body, label } => resolve_loop(resolver, body, label),
            Statement::Labeled { name, stmt } => Ok(Statement::Labeled {
                name,
                stmt: stmt.resolve(resolver)?.into(),
            }),
            Statement::Expr { expr } => Ok(Statement::Expr {
                expr: resolve_expr(resolver, expr)?,
            }),
            _ => Ok(self),
        }
    }
}

impl Resolve for Param {
    type Output = Param;

    fn resolve(self, resolver: &mut Resolver) -> Result<Self::Output, ResolverError> {
        let uniq_name = mkuniq(&self.name);

        resolver.var_map.insert(
            self.name,
            Var {
                name: uniq_name.clone(),
            },
        );

        Ok(Param {
            name: uniq_name,
            ty: self.ty,
        })
    }
}

fn resolve_fn(
    resolver: &mut Resolver,
    name: String,
    ret: Option<Type>,
    params: Vec<Param>,
    body: Statement,
) -> Result<Declaration, ResolverError> {
    let prev = std::mem::take(&mut resolver.var_map);

    let resolved_params = params
        .into_iter()
        .map(|param| param.resolve(resolver))
        .collect::<Result<Vec<_>, _>>()?;
    let resolved_body = body.resolve(resolver)?;

    resolver.var_map = prev;

    Ok(Declaration::Fn {
        name,
        ret,
        params: resolved_params,
        body: resolved_body,
    })
}

fn resolve_let(
    resolver: &mut Resolver,
    name: String,
    init: Expression,
    ty: Option<Type>,
) -> Result<Statement, ResolverError> {
    let uniq_name = mkuniq(&name);

    let resolved_init = resolve_expr(resolver, init)?;

    resolver.var_map.insert(
        name,
        Var {
            name: uniq_name.clone(),
        },
    );

    Ok(Statement::Let {
        name: uniq_name,
        init: resolved_init,
        ty,
    })
}

fn resolve_ret(
    resolver: &mut Resolver,
    value: Option<Expression>,
    ret_ty: Option<Type>,
) -> Result<Statement, ResolverError> {
    Ok(Statement::Ret {
        value: value.map(|v| resolve_expr(resolver, v)).transpose()?,
        ret_ty,
    })
}

fn resolve_print(resolver: &mut Resolver, value: Expression) -> Result<Statement, ResolverError> {
    Ok(Statement::Print {
        value: resolve_expr(resolver, value)?,
    })
}

fn resolve_if(
    resolver: &mut Resolver,
    cond: Expression,
    then_block: Box<Statement>,
    else_block: Option<Box<Statement>>,
) -> Result<Statement, ResolverError> {
    Ok(Statement::If {
        cond: resolve_expr(resolver, cond)?,
        then_block: then_block.resolve(resolver)?.into(),
        else_block: else_block
            .map(|block| block.resolve(resolver).map(Box::new))
            .transpose()?,
    })
}

fn resolve_while(
    resolver: &mut Resolver,
    cond: Expression,
    body: Box<Statement>,
    label: Option<String>,
) -> Result<Statement, ResolverError> {
    Ok(Statement::While {
        cond: resolve_expr(resolver, cond)?,
        body: body.resolve(resolver)?.into(),
        label,
    })
}

fn resolve_do_while(
    resolver: &mut Resolver,
    cond: Expression,
    body: Box<Statement>,
    label: Option<String>,
) -> Result<Statement, ResolverError> {
    Ok(Statement::DoWhile {
        cond: resolve_expr(resolver, cond)?,
        body: body.resolve(resolver)?.into(),
        label,
    })
}

fn resolve_for(
    resolver: &mut Resolver,
    init: Option<Box<Statement>>,
    cond: Option<Expression>,
    advancement: Option<Expression>,
    body: Box<Statement>,
    label: Option<String>,
) -> Result<Statement, ResolverError> {
    Ok(Statement::For {
        init: init
            .map(|i| i.resolve(resolver).map(Box::new))
            .transpose()?,
        cond: cond.map(|c| resolve_expr(resolver, c)).transpose()?,
        advancement: advancement.map(|a| resolve_expr(resolver, a)).transpose()?,
        body: body.resolve(resolver)?.into(),
        label,
    })
}

fn resolve_loop(
    resolver: &mut Resolver,
    body: Box<Statement>,
    label: Option<String>,
) -> Result<Statement, ResolverError> {
    Ok(Statement::Loop {
        body: body.resolve(resolver)?.into(),
        label,
    })
}

fn resolve_block(
    resolver: &mut Resolver,
    body: Vec<Statement>,
) -> Result<Vec<Statement>, ResolverError> {
    body.into_iter()
        .map(|stmt| stmt.resolve(resolver))
        .collect::<Result<Vec<_>, _>>()
}

fn resolve_expr(resolver: &mut Resolver, expr: Expression) -> Result<Expression, ResolverError> {
    match expr {
        Expression::Literal { val, ty, span } => Ok(Expression::Literal { val, ty, span }),

        Expression::Variable { name, ty, span } => {
            let resolved_var = resolver.var_map.get(&name).cloned();

            if resolved_var.is_none() {
                return Err(ResolverError {
                    msg: format!("undefined var: {}", name),
                });
            }

            let resolved_var = resolved_var.unwrap();

            Ok(Expression::Variable {
                name: resolved_var.name,
                ty,
                span,
            })
        }

        Expression::Unary {
            kind,
            expr,
            ty,
            span,
        } => Ok(Expression::Unary {
            kind,
            expr: resolve_expr(resolver, *expr)?.into(),
            ty,
            span,
        }),

        Expression::Binary {
            kind,
            lhs,
            rhs,
            ty,
            span,
        } => Ok(Expression::Binary {
            kind,
            lhs: resolve_expr(resolver, *lhs)?.into(),
            rhs: resolve_expr(resolver, *rhs)?.into(),
            ty,
            span,
        }),

        Expression::Call {
            target,
            args,
            ty,
            span,
        } => Ok(Expression::Call {
            target,
            args: args
                .into_iter()
                .map(|arg| resolve_expr(resolver, arg))
                .collect::<Result<Vec<_>, _>>()?,
            ty,
            span,
        }),

        Expression::Assign { lhs, rhs, ty, span } => Ok(Expression::Assign {
            lhs: resolve_expr(resolver, *lhs)?.into(),
            rhs: resolve_expr(resolver, *rhs)?.into(),
            ty,
            span,
        }),

        Expression::Cast {
            expr,
            target_ty,
            ty,
            span,
        } => Ok(Expression::Cast {
            expr: resolve_expr(resolver, *expr)?.into(),
            target_ty,
            ty,
            span,
        }),
    }
}

#[derive(Debug)]
pub struct ResolverError {
    msg: String,
}

impl std::error::Error for ResolverError {}

impl std::fmt::Display for ResolverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "resolver: {}", self.msg)
    }
}

fn mkuniq(s: &str) -> String {
    let tmp = mktmp();
    format!("var.{}.{}", s, tmp)
}
