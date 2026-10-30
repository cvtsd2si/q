use crate::{
    ir::r#gen::{IRFunc, IRInstr, IRVal},
    parser::ast::{BinaryExpressionKind, Literal, Type, UnaryExpressionKind},
    runtime::value::Value,
};

pub(super) fn run(func: &mut IRFunc) -> bool {
    let mut changed = false;
    let mut result = Vec::with_capacity(func.body.len());

    for instr in func.body.drain(..) {
        match instr {
            IRInstr::Bin {
                kind,
                lhs: IRVal::Const(a),
                rhs: IRVal::Const(b),
                dst,
                operand_ty,
            } if Into::<Type>::into(a) == operand_ty && Into::<Type>::into(b) == operand_ty => {
                if let Some(folded) = fold_binary(kind, a, b) {
                    changed = true;
                    result.push(IRInstr::Cpy {
                        src: IRVal::Const(folded),
                        src_ty: folded.into(),
                        dst,
                    });
                } else {
                    result.push(IRInstr::Bin {
                        kind,
                        lhs: IRVal::Const(a),
                        rhs: IRVal::Const(b),
                        dst,
                        operand_ty,
                    });
                }
            }

            IRInstr::Unary {
                kind,
                src: IRVal::Const(a),
                dst,
                operand_ty,
            } if Into::<Type>::into(a) == operand_ty => {
                if let Some(folded) = fold_unary(kind, a) {
                    changed = true;
                    result.push(IRInstr::Cpy {
                        src: IRVal::Const(folded),
                        src_ty: folded.into(),
                        dst,
                    });
                } else {
                    result.push(IRInstr::Unary {
                        kind,
                        src: IRVal::Const(a),
                        dst,
                        operand_ty,
                    });
                }
            }

            IRInstr::Jz {
                cond: IRVal::Const(a),
                target,
            } => {
                changed = true;
                if Value::from(a).payload == 0 {
                    result.push(IRInstr::Jmp { target });
                }
                /* Nonzero condition falls through; no instruction is needed.  */
            }

            other => {
                /* Conversion folding is handled here too.  */
                if let Some(replacement) = fold_cast(&other) {
                    changed = true;
                    result.push(replacement);
                } else {
                    result.push(other);
                }
            }
        }
    }

    func.body = result;

    changed
}

fn fold_binary(kind: BinaryExpressionKind, a: Literal, b: Literal) -> Option<Literal> {
    let a_ty: Type = a.into();
    let b_ty: Type = b.into();

    if a_ty != b_ty {
        return None;
    }

    /* From here we can assume that a_ty == b_ty */

    let is_numeric = a_ty.is_numeric();
    let is_int = a_ty.is_int();
    let is_bool = matches!(a_ty, Type::Boolean);

    let av = Value::from(a);
    let bv = Value::from(b);

    /* Do not crash the compiler because an expression would fail at runtime.  */
    if is_int
        && bv.payload == 0
        && matches!(kind, BinaryExpressionKind::Div | BinaryExpressionKind::Rem)
    {
        return None;
    }

    let result: Value = match kind {
        BinaryExpressionKind::Add if is_numeric => av + bv,
        BinaryExpressionKind::Sub if is_numeric => av - bv,
        BinaryExpressionKind::Mul if is_numeric => av * bv,
        BinaryExpressionKind::Div if is_numeric => av / bv,
        BinaryExpressionKind::Rem if is_int => av % bv,
        BinaryExpressionKind::Shl if is_int => av << bv,
        BinaryExpressionKind::Shr if is_int => av >> bv,
        BinaryExpressionKind::BitwiseAnd if is_int => av & bv,
        BinaryExpressionKind::BitwiseOr if is_int => av | bv,
        BinaryExpressionKind::BitwiseXor if is_int => av ^ bv,
        BinaryExpressionKind::And if is_bool => ((av.payload != 0) && (bv.payload != 0)).into(),
        BinaryExpressionKind::Or if is_bool => ((av.payload != 0) || (bv.payload != 0)).into(),
        BinaryExpressionKind::Equal => (av == bv).into(),
        BinaryExpressionKind::NotEqual => (av != bv).into(),
        BinaryExpressionKind::Less if is_numeric => (av < bv).into(),
        BinaryExpressionKind::Greater if is_numeric => (av > bv).into(),
        BinaryExpressionKind::LessEqual if is_numeric => (av <= bv).into(),
        BinaryExpressionKind::GreaterEqual if is_numeric => (av >= bv).into(),
        _ => return None,
    };

    Some(result.into())
}

fn fold_unary(kind: UnaryExpressionKind, value: Literal) -> Option<Literal> {
    let v = Value::from(value);

    match kind {
        UnaryExpressionKind::Negate
            if matches!(
                value,
                Literal::Int8(_)
                    | Literal::Int16(_)
                    | Literal::Int32(_)
                    | Literal::Int64(_)
                    | Literal::Float32(_)
                    | Literal::Float64(_)
            ) =>
        {
            Some((-v).into())
        }

        UnaryExpressionKind::BitwiseNot
            if !matches!(value, Literal::Float32(_) | Literal::Float64(_)) =>
        {
            Some((!v).into())
        }

        UnaryExpressionKind::LogicalNot if matches!(value, Literal::Bool(_)) => Some((!v).into()),
        _ => None,
    }
}

fn fold_cast(instr: &IRInstr) -> Option<IRInstr> {
    let (src, dst, src_ty, dst_ty) = match instr {
        IRInstr::SignExtend {
            src,
            dst,
            src_ty,
            dst_ty,
        }
        | IRInstr::ZeroExtend {
            src,
            dst,
            src_ty,
            dst_ty,
        }
        | IRInstr::Truncate {
            src,
            dst,
            src_ty,
            dst_ty,
        }
        | IRInstr::DoubleToInt {
            src,
            dst,
            src_ty,
            dst_ty,
        }
        | IRInstr::IntToDouble {
            src,
            dst,
            src_ty,
            dst_ty,
        }
        | IRInstr::DoubleToUInt {
            src,
            dst,
            src_ty,
            dst_ty,
        }
        | IRInstr::UIntToDouble {
            src,
            dst,
            src_ty,
            dst_ty,
        }
        | IRInstr::DoubleToFloat {
            src,
            dst,
            src_ty,
            dst_ty,
        }
        | IRInstr::FloatToDouble {
            src,
            dst,
            src_ty,
            dst_ty,
        }
        | IRInstr::FloatToInt {
            src,
            dst,
            src_ty,
            dst_ty,
        }
        | IRInstr::IntToFloat {
            src,
            dst,
            src_ty,
            dst_ty,
        }
        | IRInstr::FloatToUInt {
            src,
            dst,
            src_ty,
            dst_ty,
        }
        | IRInstr::UIntToFloat {
            src,
            dst,
            src_ty,
            dst_ty,
        } => (src, dst, *src_ty, *dst_ty),
        _ => return None,
    };

    let IRVal::Const(a) = src else { return None };
    let folded = cast_literal(*a, src_ty, dst_ty)?;

    Some(IRInstr::Cpy {
        src: IRVal::Const(folded),
        src_ty: dst_ty,
        dst: dst.clone(),
    })
}

fn cast_literal(value: Literal, src_ty: Type, dst_ty: Type) -> Option<Literal> {
    let v: Value = value.into();

    Some((v.cast(src_ty, dst_ty)).into())
}
