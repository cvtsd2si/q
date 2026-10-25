use std::collections::HashMap;

use crate::lexer::token::Span;
use crate::parser::ast::{
    BinaryExpressionKind, BlockItem, Declaration, Expression, Literal, LiteralVal, Param, Program,
    Statement, Type, UnaryExpressionKind,
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

impl Default for Typechecker {
    fn default() -> Self {
        Self::new()
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
            if let BlockItem::Decl(decl) = block_item
                && let Declaration::Fn {
                    name,
                    ret,
                    params,
                    body: _,
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

            Statement::DoWhile { cond, body, label } => {
                typecheck_do_while(typechecker, cond, body, label)
            }

            Statement::For {
                init,
                cond,
                advancement,
                body,
                label,
            } => typecheck_for(typechecker, init, cond, advancement, body, label),

            Statement::Loop { body, label } => typecheck_loop(typechecker, body, label),

            Statement::Labeled { name, stmt } => Ok(Statement::Labeled {
                name,
                stmt: stmt.typecheck(typechecker)?.into(),
            }),

            Statement::Expr { expr } => Ok(Self::Expr {
                expr: typecheck_expr(typechecker, expr, None)?,
            }),

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
    let old_sym_table = std::mem::take(&mut typechecker.sym_table);

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
            ty,
        },
    );

    Ok(Statement::Let {
        name,
        init: typechecked_init,
        ty,
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

    if let Some(expected) = expected_ty
        && actual_ty != Some(expected)
    {
        return Err(TypecheckerError {
            msg: format!(
                "type mismatch: got {:?} and wanted {:?}",
                actual_ty, expected_ty
            ),
        });
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

fn typecheck_do_while(
    typechecker: &mut Typechecker,
    cond: Expression,
    body: Box<Statement>,
    label: Option<String>,
) -> Result<Statement, TypecheckerError> {
    Ok(Statement::DoWhile {
        cond: typecheck_expr(typechecker, cond, Some(Type::Boolean))?,
        body: body.typecheck(typechecker)?.into(),
        label,
    })
}

fn typecheck_for(
    typechecker: &mut Typechecker,
    init: Option<Box<Statement>>,
    cond: Option<Expression>,
    advancement: Option<Expression>,
    body: Box<Statement>,
    label: Option<String>,
) -> Result<Statement, TypecheckerError> {
    Ok(Statement::For {
        init: init
            .map(|i| i.typecheck(typechecker).map(Box::new))
            .transpose()?,
        cond: cond
            .map(|c| typecheck_expr(typechecker, c, Some(Type::Boolean)))
            .transpose()?,
        advancement: advancement
            .map(|a| typecheck_expr(typechecker, a, None))
            .transpose()?,
        body: body.typecheck(typechecker)?.into(),
        label,
    })
}

fn typecheck_loop(
    typechecker: &mut Typechecker,
    body: Box<Statement>,
    label: Option<String>,
) -> Result<Statement, TypecheckerError> {
    Ok(Statement::Loop {
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

        Expression::Variable { name, ty: _, span } => {
            let sym = typechecker.sym_table.get(&name);

            if sym.is_none() {
                return Err(TypecheckerError {
                    msg: format!("Referenced an unknown variable: {:?}.", sym),
                });
            }

            let sym = sym.unwrap();

            if sym.ty.is_none() {
                return Err(TypecheckerError {
                    msg: "Referenced an unknown type.".to_string(),
                });
            }

            Ok(Expression::Variable {
                name,
                ty: sym.ty,
                span,
            })
        }

        Expression::Unary {
            kind,
            expr,
            ty: _,
            span,
        } => {
            if matches!(kind, UnaryExpressionKind::Negate) {
                if let Expression::Literal {
                    val: LiteralVal::Integer(n),
                    ty,
                    ..
                } = expr.as_ref()
                {
                    return typecheck_literal(
                        LiteralVal::NegativeInteger(*n),
                        *ty,
                        expected_ty,
                        span,
                    );
                }
            }

            let op_hint = known_ty(typechecker, &expr)?;

            let operand_ty = match kind {
                UnaryExpressionKind::LogicalNot => Type::Boolean,
                UnaryExpressionKind::Negate | UnaryExpressionKind::BitwiseNot => {
                    merge_types(expected_ty, op_hint)?.unwrap_or_else(|| default_numeric_ty(&expr))
                }
            };

            let expr = typecheck_expr(typechecker, *expr, Some(operand_ty))?;

            if expr.ty() != Some(operand_ty) {
                return Err(TypecheckerError {
                    msg: format!(
                        "Expected unary expr of type {operand_ty:?}, got {:?}",
                        expr.ty()
                    ),
                });
            }

            let is_valid_ty = match kind {
                UnaryExpressionKind::Negate => matches!(
                    operand_ty,
                    Type::Int8
                        | Type::Int16
                        | Type::Int32
                        | Type::Int64
                        | Type::Float32
                        | Type::Float64
                ),
                UnaryExpressionKind::LogicalNot => operand_ty == Type::Boolean,
                UnaryExpressionKind::BitwiseNot => matches!(
                    operand_ty,
                    Type::UInt8
                        | Type::Int8
                        | Type::UInt16
                        | Type::Int16
                        | Type::UInt32
                        | Type::Int32
                        | Type::UInt64
                        | Type::Int64
                        | Type::Float32
                        | Type::Float64
                ),
            };

            if !is_valid_ty {
                return Err(TypecheckerError {
                    msg: format!("Invalid operand type {operand_ty:?} for unary {kind:?}"),
                });
            }

            let result_ty = match kind {
                UnaryExpressionKind::LogicalNot => Type::Boolean,
                UnaryExpressionKind::Negate | UnaryExpressionKind::BitwiseNot => operand_ty,
            };

            if let Some(expected) = expected_ty
                && expected != result_ty
            {
                return Err(TypecheckerError {
                    msg: format!("Expected {expected:?}, got {result_ty:?}."),
                });
            }

            Ok(Expression::Unary {
                kind,
                expr: expr.into(),
                ty: Some(result_ty),
                span,
            })
        }

        Expression::Binary {
            kind,
            lhs,
            rhs,
            ty: _,
            span,
        } => {
            let lhs_hint = known_ty(typechecker, &lhs)?;
            let rhs_hint = known_ty(typechecker, &rhs)?;

            let op_hint = merge_types(lhs_hint, rhs_hint)?;

            let is_comparison = matches!(
                kind,
                BinaryExpressionKind::Less
                    | BinaryExpressionKind::Greater
                    | BinaryExpressionKind::LessEqual
                    | BinaryExpressionKind::GreaterEqual
                    | BinaryExpressionKind::Equal
                    | BinaryExpressionKind::NotEqual
            );

            let operand_ty = if is_comparison {
                op_hint
            } else {
                merge_types(expected_ty, op_hint)?
            }
            .unwrap_or_else(|| default_numeric_ty(&lhs));

            if !operand_ty.is_numeric() {
                return Err(TypecheckerError {
                    msg: format!("Arithmetic requires numeric operands, got {operand_ty:?}"),
                });
            }

            let lhs = typecheck_expr(typechecker, *lhs, Some(operand_ty))?;
            let rhs = typecheck_expr(typechecker, *rhs, Some(operand_ty))?;

            if lhs.ty() != Some(operand_ty) || rhs.ty() != Some(operand_ty) {
                return Err(TypecheckerError {
                    msg: format!("invalid operands: {:?} and {:?}", lhs.ty(), rhs.ty()),
                });
            }

            let result_ty = if is_comparison {
                Type::Boolean
            } else {
                operand_ty
            };

            Ok(Expression::Binary {
                kind,
                lhs: lhs.into(),
                rhs: rhs.into(),
                ty: Some(result_ty),
                span,
            })
        }

        Expression::Call {
            target,
            args,
            ty: _,
            span,
        } => {
            let s = typechecker
                .funcs
                .get(&target)
                .cloned()
                .ok_or_else(|| TypecheckerError {
                    msg: format!("Unknown fn: {target}"),
                })?;

            if let Some(expected) = expected_ty
                && s.ret_ty != Some(expected)
            {
                let t = s.ret_ty;
                return Err(TypecheckerError {
                    msg: format!("Expected {expected:?}, but fn {target} returns {t:?}"),
                });
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

        Expression::Assign { lhs, rhs, span, .. } => {
            /* Ensure lhs is assignable.  */
            if !matches!(lhs.as_ref(), Expression::Variable { .. }) {
                return Err(TypecheckerError {
                    msg: "lhs must be assignable".to_string(),
                });
            }

            let lhs = typecheck_expr(typechecker, *lhs, None)?;

            let lhs_ty = lhs.ty().ok_or_else(|| TypecheckerError {
                msg: "cannot assign to a var with unknown ty".to_string(),
            })?;

            if let Some(expected) = expected_ty
                && expected != lhs_ty
            {
                return Err(TypecheckerError {
                    msg: format!("expected assignment of type {expected:?}, got {lhs_ty:?}"),
                });
            }

            /* The rhs must have the same type as lhs.  */
            let rhs = typecheck_expr(typechecker, *rhs, Some(lhs_ty))?;

            if rhs.ty() != Some(lhs_ty) {
                return Err(TypecheckerError {
                    msg: format!(
                        "cannot assign {:?} to a variable of type {lhs_ty:?}",
                        rhs.ty()
                    ),
                });
            }

            Ok(Expression::Assign {
                lhs: lhs.into(),
                rhs: rhs.into(),
                ty: Some(lhs_ty),
                span,
            })
        }

        Expression::Cast {
            expr,
            target_ty,
            ty: _,
            span,
        } => {
            /* Determine the operand's own type independently of the destination type.  */
            let expr = typecheck_expr(typechecker, *expr, None)?;

            let src_ty = expr.ty().ok_or_else(|| TypecheckerError {
                msg: "Cannot determine the type of cast operand".into(),
            })?;

            /* FIXME: Only allow casts between numeric types for now.  */
            if !src_ty.is_numeric() || !target_ty.is_numeric() {
                return Err(TypecheckerError {
                    msg: format!("Invalid cast from {src_ty:?} to {target_ty:?}."),
                });
            }

            /* The cast produces `target_ty`, regardless of `src_ty`.  */
            if let Some(expected) = expected_ty {
                if expected != target_ty {
                    return Err(TypecheckerError {
                        msg: format!("Expected {expected:?}, got {target_ty:?}."),
                    });
                }
            }

            Ok(Expression::Cast {
                expr: expr.into(),
                target_ty,
                ty: Some(target_ty),
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
    if let (Some(explicit), Some(expected)) = (explicit_ty, expected_ty)
        && explicit != expected
    {
        return Err(TypecheckerError {
            msg: format!("got: {explicit:?}, expected: {expected:?}"),
        });
    }

    let ty = expected_ty.or(explicit_ty).unwrap_or(match &val {
        LiteralVal::Bool(_) => Type::Boolean,
        LiteralVal::Integer(_) | LiteralVal::NegativeInteger(_) => Type::Int32,
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

    if let Some(expected) = expected_ty
        && actual_ty != Some(expected)
    {
        return Err(TypecheckerError {
            msg: format!("Expected {:?}, got {:?}", expected_ty, actual_ty),
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
                ty: self.ty.into(),
            },
        );

        Ok(Param {
            name: self.name.clone(),
            ty: self.ty,
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

        Expression::Unary { kind, expr, .. } => match kind {
            UnaryExpressionKind::LogicalNot => Ok(Some(Type::Boolean)),
            UnaryExpressionKind::BitwiseNot | UnaryExpressionKind::Negate => {
                known_ty(typechecker, expr)
            }
        },

        Expression::Binary { kind, lhs, rhs, .. } => {
            if matches!(
                kind,
                BinaryExpressionKind::Less
                    | BinaryExpressionKind::Greater
                    | BinaryExpressionKind::LessEqual
                    | BinaryExpressionKind::GreaterEqual
                    | BinaryExpressionKind::Equal
                    | BinaryExpressionKind::NotEqual
            ) {
                Ok(Some(Type::Boolean))
            } else {
                merge_types(known_ty(typechecker, lhs)?, known_ty(typechecker, rhs)?)
            }
        }

        Expression::Assign { lhs, .. } => {
            if !matches!(lhs.as_ref(), Expression::Variable { .. }) {
                return Err(TypecheckerError {
                    msg: "lhs of assign expr must be a var".to_string(),
                });
            }

            known_ty(typechecker, lhs)
        }

        Expression::Cast {
            target_ty,
            ..
        } => Ok(Some(*target_ty)),
    }
}

fn default_numeric_ty(expr: &Expression) -> Type {
    match expr {
        Expression::Literal {
            val: LiteralVal::Float(_),
            ..
        } => Type::Float64,
        Expression::Unary { expr, .. } => default_numeric_ty(expr),
        Expression::Binary { lhs, .. } => default_numeric_ty(lhs),
        Expression::Cast { target_ty, .. } => *target_ty,
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

        Statement::Labeled { stmt, .. } => always_returns(stmt),

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
