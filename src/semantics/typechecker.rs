use std::collections::HashMap;

use crate::lexer::token::Span;
use crate::parser::ast::{
    BlockItem, Declaration, Expression, Literal, LiteralVal, Param, Program, Statement, Type,
};

pub struct Typechecker {
    pub sym_table: HashMap<String, Symbol>,
    pub funcs: HashMap<String, FnSignature>,
}

impl Typechecker {
    pub fn new() -> Self {
        Self {
            sym_table: HashMap::new(),
            funcs: HashMap::new(),
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
        self.block_items.iter().for_each(|block_item| {
            if let BlockItem::Decl(decl) = block_item {
                if let Declaration::Fn {
                    name,
                    ret,
                    params,
                    body,
                } = decl
                {
                    typechecker.funcs.insert(
                        name.to_owned(),
                        FnSignature {
                            params: params.iter().map(|p| p.ty).collect(),
                            ret_ty: *ret,
                        },
                    );
                }
            }
        });

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

            Statement::Ret { value, ret_ty } => typecheck_ret(typechecker, value, ret_ty),

            Statement::Print { value } => typecheck_print(typechecker, value),

            Statement::If {
                cond,
                then_block,
                else_block,
            } => typecheck_if(typechecker, cond, then_block, else_block),

            Statement::Block { body } => Ok(Statement::Block {
                body: typecheck_block(typechecker, body)?,
            }),

            Statement::While { cond, body, label } => {
                typecheck_while(typechecker, cond, body, label)
            }

            _ => Ok(self),
        }
    }
}

fn typecheck_fn(
    typechecker: &mut Typechecker,
    name: String,
    ret: Option<Type>,
    params: Vec<Param>,
    body: Statement,
) -> Result<Declaration, TypecheckerError> {
    let old_sym_table = std::mem::replace(&mut typechecker.sym_table, HashMap::new());

    let typechecked_params = params
        .into_iter()
        .map(|param| param.typecheck(typechecker))
        .collect::<Result<Vec<_>, _>>()?;
    let typechecked_body = body.typecheck(typechecker)?;

    if ret.is_some() && !always_returns(&typechecked_body) {
        return Err(TypecheckerError {
            msg: format!("fn {name} doesn't return on every control path"),
        });
    }

    typechecker.sym_table = old_sym_table;

    Ok(Declaration::Fn {
        name: name.clone(),
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
    value: Option<Expression>,
    ret_ty: Option<Type>,
) -> Result<Statement, TypecheckerError> {
    if value.is_none() {
        return Ok(Statement::Ret { value, ret_ty });
    }

    let typechecked_expr = typecheck_expr(typechecker, value.unwrap(), ret_ty)?;

    let actual_ty = typechecked_expr.ty();
    let expected_ty = ret_ty;

    if let Some(expected) = expected_ty {
        if actual_ty != Some(expected) {
            return Err(TypecheckerError {
                msg: format!(
                    "type mismatch: got {:?} and wanted {:?}",
                    actual_ty, expected_ty
                ),
            });
        }
    }

    Ok(Statement::Ret {
        value: Some(typechecked_expr),
        ret_ty: expected_ty,
    })
}

fn typecheck_print(
    typechecker: &mut Typechecker,
    value: Expression,
) -> Result<Statement, TypecheckerError> {
    Ok(Statement::Print {
        value: typecheck_expr(typechecker, value, None)?,
    })
}

fn typecheck_if(
    typechecker: &mut Typechecker,
    cond: Expression,
    then_block: Box<Statement>,
    else_block: Option<Box<Statement>>,
) -> Result<Statement, TypecheckerError> {
    Ok(Statement::If {
        cond: typecheck_expr(typechecker, cond, Some(Type::Boolean))?,
        then_block: then_block.typecheck(typechecker)?.into(),
        else_block: else_block
            .map(|block| block.typecheck(typechecker).map(Box::new))
            .transpose()?,
    })
}

fn typecheck_while(
    typechecker: &mut Typechecker,
    cond: Expression,
    body: Box<Statement>,
    label: Option<String>,
) -> Result<Statement, TypecheckerError> {
    Ok(Statement::While {
        cond: typecheck_expr(typechecker, cond, Some(Type::Boolean))?,
        body: body.typecheck(typechecker)?.into(),
        label,
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
    expected_ty: Option<Type>,
) -> Result<Expression, TypecheckerError> {
    match expr {
        Expression::Literal { val, ty, span } => typecheck_literal(val, ty, expected_ty, span),

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
        } => {
            let lhs_hint = known_ty(typechecker, &lhs)?;
            let rhs_hint = known_ty(typechecker, &rhs)?;

            let op_hint = merge_types(lhs_hint, rhs_hint)?;

            let resolved = merge_types(expected_ty, op_hint)?;

            let ty = resolved.unwrap_or_else(|| default_numeric_ty(&lhs));

            if !ty.is_numeric() {
                return Err(TypecheckerError {
                    msg: format!("Arithmetic requires numeric operands, got {ty:?}"),
                });
            }

            let lhs = typecheck_expr(typechecker, *lhs, Some(ty))?;
            let rhs = typecheck_expr(typechecker, *rhs, Some(ty))?;

            if lhs.ty() != Some(ty) || rhs.ty() != Some(ty) {
                return Err(TypecheckerError {
                    msg: format!("invalid operands: {:?} and {:?}", lhs.ty(), rhs.ty()),
                });
            }

            Ok(Expression::Binary {
                kind,
                lhs: lhs.into(),
                rhs: rhs.into(),
                ty: Some(ty),
                span,
            })
        }

        Expression::Call {
            target,
            args,
            ty,
            span,
        } => {
            let s = typechecker
                .funcs
                .get(&target)
                .cloned()
                .ok_or_else(|| TypecheckerError {
                    msg: format!("Unknown fn: {target}"),
                })?;

            if let Some(expected) = expected_ty {
                if s.ret_ty != Some(expected) {
                    let t = s.ret_ty.clone();
                    return Err(TypecheckerError {
                        msg: format!("Expected {expected:?}, but fn {target} returns {t:?}"),
                    });
                }
            }

            if args.len() != s.params.len() {
                return Err(TypecheckerError {
                    msg: format!(
                        "Fn {target} expects {} args, got {}",
                        s.params.len(),
                        args.len()
                    ),
                });
            }

            let typechecked_args = args
                .into_iter()
                .zip(s.params.iter().copied())
                .enumerate()
                .map(|(i, (arg, param_ty))| {
                    let arg = typecheck_expr(typechecker, arg, Some(param_ty))?;

                    if arg.ty() != Some(param_ty) {
                        return Err(TypecheckerError {
                            msg: format!(
                                "arg {} of {target:?}: expected {:?}, got {:?}",
                                i + 1,
                                param_ty,
                                arg.ty()
                            ),
                        });
                    }

                    Ok(arg)
                })
                .collect::<Result<Vec<_>, _>>()?;

            Ok(Expression::Call {
                target,
                args: typechecked_args,
                ty: s.ret_ty,
                span,
            })
        }
    }
}

fn typecheck_literal(
    val: LiteralVal,
    explicit_ty: Option<Type>,
    expected_ty: Option<Type>,
    span: Span,
) -> Result<Expression, TypecheckerError> {
    if let (Some(explicit), Some(expected)) = (explicit_ty, expected_ty) {
        if explicit != expected {
            return Err(TypecheckerError {
                msg: format!("got: {explicit:?}, expected: {expected:?}"),
            });
        }
    }

    let ty = expected_ty.or(explicit_ty).unwrap_or_else(|| match &val {
        LiteralVal::Bool(_) => Type::Boolean,
        LiteralVal::Integer(_) => Type::Int32,
        LiteralVal::Float(_) => Type::Float64,
    });

    Literal::try_from((&val, ty)).map_err(|msg| TypecheckerError { msg })?;

    Ok(Expression::Literal {
        val,
        ty: Some(ty),
        span,
    })
}

fn typecheck_init(
    typechecker: &mut Typechecker,
    init: Expression,
    expected_ty: Option<Type>,
) -> Result<Expression, TypecheckerError> {
    let typechecked_init = typecheck_expr(typechecker, init.clone(), expected_ty)?;

    let actual_ty = typechecked_init.ty();

    if let Some(expected) = expected_ty {
        if actual_ty != Some(expected) {
            return Err(TypecheckerError {
                msg: format!("Expected {:?}, got {:?}", expected_ty, actual_ty),
            });
        }
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

fn merge_types(a: Option<Type>, b: Option<Type>) -> Result<Option<Type>, TypecheckerError> {
    match (a, b) {
        (Some(a), Some(b)) if a != b => Err(TypecheckerError {
            msg: format!("{a:?} != {b:?}"),
        }),

        (Some(ty), _) | (_, Some(ty)) => Ok(Some(ty)),

        (None, None) => Ok(None),
    }
}

fn known_ty(
    typechecker: &mut Typechecker,
    expr: &Expression,
) -> Result<Option<Type>, TypecheckerError> {
    match expr {
        Expression::Literal { ty, .. } => Ok(*ty),

        Expression::Variable { name, .. } => typechecker
            .sym_table
            .get(name)
            .map(|sym| sym.ty)
            .ok_or_else(|| TypecheckerError {
                msg: format!("Unknown var: {name}"),
            }),

        Expression::Call { target, .. } => typechecker
            .funcs
            .get(target)
            .map(|sig| sig.ret_ty)
            .ok_or_else(|| TypecheckerError {
                msg: format!("Unknown fn: {target}"),
            }),

        Expression::Binary { lhs, rhs, .. } => {
            merge_types(known_ty(typechecker, lhs)?, known_ty(typechecker, rhs)?)
        }
    }
}

fn default_numeric_ty(expr: &Expression) -> Type {
    match expr {
        Expression::Literal {
            val: LiteralVal::Float(_),
            ..
        } => Type::Float64,
        Expression::Binary { lhs, .. } => default_numeric_ty(lhs),
        _ => Type::Int32,
    }
}

fn always_returns(stmt: &Statement) -> bool {
    match stmt {
        Statement::Ret { .. } => true,

        Statement::Block { body } => body.iter().any(always_returns),

        Statement::If {
            then_block,
            else_block: Some(else_block),
            ..
        } => always_returns(then_block) && always_returns(else_block),

        _ => false,
    }
}

#[derive(Debug)]
pub struct Symbol {
    pub name: String,
    pub ty: Option<Type>,
}

#[derive(Debug, Clone)]
pub struct FnSignature {
    pub params: Vec<Type>,
    pub ret_ty: Option<Type>,
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
