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
            Statement::Ret { value } => resolve_ret(resolver, value),
            Statement::Print { value } => resolve_print(resolver, value),
            Statement::If {
                cond,
                then_block,
                else_block,
            } => resolve_if(resolver, cond, then_block, else_block),
            Statement::Block { body } => Ok(Statement::Block {
                body: resolve_block(resolver, body)?,
            }),
        }
    }
}

fn resolve_fn(
    resolver: &mut Resolver,
    name: String,
    ret: Type,
    params: Vec<Param>,
    body: Statement,
) -> Result<Declaration, ResolverError> {
    let resolved_body = body.resolve(resolver)?;
    Ok(Declaration::Fn {
        name,
        ret,
        params,
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

    resolver.var_map.insert(
        name,
        Var {
            name: uniq_name.clone(),
        },
    );

    Ok(Statement::Let {
        name: uniq_name,
        init: resolve_expr(resolver, init)?,
        ty,
    })
}

fn resolve_ret(resolver: &mut Resolver, value: Expression) -> Result<Statement, ResolverError> {
    Ok(Statement::Ret {
        value: resolve_expr(resolver, value)?,
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
            let resolved_name = resolver.var_map.get(&name);
            if resolved_name.is_none() {
                return Err(ResolverError {
                    msg: format!("undefined var: {}", name),
                });
            }

            Ok(Expression::Variable {
                name: resolved_name.cloned().unwrap().name,
                ty,
                span,
            })
        }

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
