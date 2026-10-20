use std::collections::HashMap;

use crate::{
    parser::ast::{BlockItem, Declaration, Expression, Param, Program, Statement, Type},
    semantics::typechecker,
};

pub struct Typechecker {
    pub sym_table: HashMap<String, Symbol>,
}

impl Typechecker {
    pub fn new() -> Self {
        Self {
            sym_table: HashMap::new(),
        }
    }
}

pub trait Typecheck {
    type Output;

    fn typecheck(self, typechecker: &mut Typechecker) -> Result<Self::Output, TypecheckerError>;
}

impl Typecheck for Program {
    type Output = Program;

    fn typecheck(self, typechecker: &mut Typechecker) -> Result<Self::Output, TypecheckerError> {
        Ok(Program {
            block_items: self
                .block_items
                .into_iter()
                .map(|block_item| block_item.typecheck(typechecker))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl Typecheck for BlockItem {
    type Output = BlockItem;

    fn typecheck(self, typechecker: &mut Typechecker) -> Result<Self::Output, TypecheckerError> {
        match self {
            BlockItem::Decl(decl) => Ok(BlockItem::Decl(decl.typecheck(typechecker)?)),
            BlockItem::Stmt(stmt) => Ok(BlockItem::Stmt(stmt.typecheck(typechecker)?)),
        }
    }
}

impl Typecheck for Declaration {
    type Output = Declaration;

    fn typecheck(self, typechecker: &mut Typechecker) -> Result<Self::Output, TypecheckerError> {
        match self {
            Declaration::Fn {
                name,
                ret,
                params,
                body,
            } => typecheck_fn(typechecker, name, ret, params, body),
        }
    }
}

impl Typecheck for Statement {
    type Output = Statement;

    fn typecheck(self, typechecker: &mut Typechecker) -> Result<Self::Output, TypecheckerError> {
        match self {
            Statement::Let { name, init, ty } => typecheck_let(typechecker, name, init, ty),

            Statement::Ret { value } => typecheck_ret(typechecker, value),

            Statement::Print { value } => typecheck_print(typechecker, value),

            Statement::If {
                cond,
                then_block,
                else_block,
            } => typecheck_if(typechecker, cond, then_block, else_block),

            Statement::Block { body } => Ok(Statement::Block {
                body: typecheck_block(typechecker, body)?,
            }),
        }
    }
}

fn typecheck_fn(
    typechecker: &mut Typechecker,
    name: String,
    ret: Type,
    params: Vec<Param>,
    body: Statement,
) -> Result<Declaration, TypecheckerError> {
    typechecker.sym_table.insert(
        name.clone(),
        Symbol {
            name: name.clone(),
            ty: Some(ret),
        },
    );

    let typechecked_params = params
        .into_iter()
        .map(|param| param.typecheck(typechecker))
        .collect::<Result<Vec<_>, _>>()?;
    let typechecked_body = body.typecheck(typechecker)?;

    Ok(Declaration::Fn {
        name,
        ret,
        params: typechecked_params,
        body: typechecked_body,
    })
}

fn typecheck_let(
    typechecker: &mut Typechecker,
    name: String,
    init: Expression,
    ty: Option<Type>,
) -> Result<Statement, TypecheckerError> {
    let typechecked_init = typecheck_init(typechecker, init, ty)?;

    typechecker.sym_table.insert(
        name.clone(),
        Symbol {
            name: name.clone(),
            ty: ty.clone(),
        },
    );

    Ok(Statement::Let {
        name,
        init: typechecked_init,
        ty: ty.clone(),
    })
}

fn typecheck_ret(
    typechecker: &mut Typechecker,
    value: Expression,
) -> Result<Statement, TypecheckerError> {
    Ok(Statement::Ret {
        value: typecheck_expr(typechecker, value)?,
    })
}

fn typecheck_print(
    typechecker: &mut Typechecker,
    value: Expression,
) -> Result<Statement, TypecheckerError> {
    Ok(Statement::Print {
        value: typecheck_expr(typechecker, value)?,
    })
}

fn typecheck_if(
    typechecker: &mut Typechecker,
    cond: Expression,
    then_block: Box<Statement>,
    else_block: Option<Box<Statement>>,
) -> Result<Statement, TypecheckerError> {
    Ok(Statement::If {
        cond: typecheck_expr(typechecker, cond)?,
        then_block: then_block.typecheck(typechecker)?.into(),
        else_block: else_block
            .map(|block| block.typecheck(typechecker).map(Box::new))
            .transpose()?,
    })
}

fn typecheck_block(
    typechecker: &mut Typechecker,
    body: Vec<Statement>,
) -> Result<Vec<Statement>, TypecheckerError> {
    body.into_iter()
        .map(|stmt| stmt.typecheck(typechecker))
        .collect()
}

fn typecheck_expr(
    typechecker: &mut Typechecker,
    expr: Expression,
) -> Result<Expression, TypecheckerError> {
    match expr {
        Expression::Literal { val, ty, span } => Ok(Expression::Literal { val, ty, span }),

        Expression::Variable { name, ty, span } => {
            let sym = typechecker.sym_table.get(&name);

            if sym.is_none() {
                return Err(TypecheckerError {
                    msg: format!("Referenced an unknown variable: {:?}.", sym),
                });
            }

            let sym = sym.unwrap();

            if sym.ty.is_none() {
                return Err(TypecheckerError {
                    msg: format!("Referenced an unknown type."),
                });
            }

            Ok(Expression::Variable {
                name,
                ty: sym.ty,
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
            lhs: typecheck_expr(typechecker, *lhs)?.into(),
            rhs: typecheck_expr(typechecker, *rhs)?.into(),
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
                .map(|arg| typecheck_expr(typechecker, arg.to_owned()))
                .collect::<Result<Vec<_>, _>>()?,
            ty,
            span,
        }),
    }
}

fn typecheck_init(
    typechecker: &mut Typechecker,
    init: Expression,
    expected_ty: Option<Type>,
) -> Result<Expression, TypecheckerError> {
    let typechecked_init = typecheck_expr(typechecker, init.clone())?;

    let actual_ty = typechecked_init.ty();

    if !types_equal(actual_ty, expected_ty) {
        return Err(TypecheckerError {
            msg: format!("something wrong with the types"),
        });
    }

    Ok(typechecked_init)
}

impl Typecheck for Param {
    type Output = Param;

    fn typecheck(self, typechecker: &mut Typechecker) -> Result<Self::Output, TypecheckerError> {
        typechecker.sym_table.insert(
            self.name.clone(),
            Symbol {
                name: self.name.clone(),
                ty: self.ty.clone().into(),
            },
        );

        Ok(Param {
            name: self.name.clone(),
            ty: self.ty.clone(),
        })
    }
}

fn types_equal(a: Option<Type>, b: Option<Type>) -> bool {
    a == b
}

#[derive(Debug)]
pub struct Symbol {
    pub name: String,
    pub ty: Option<Type>,
}

#[derive(Debug)]
pub struct TypecheckerError {
    msg: String,
}

impl std::error::Error for TypecheckerError {}

impl std::fmt::Display for TypecheckerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "typechecker: {}", self.msg)
    }
}
