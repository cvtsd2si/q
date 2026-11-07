use std::sync::atomic::AtomicU64;

use crate::parser::ast::{
    BinaryExpressionKind, BlockItem, Declaration, Expression, GetExpressionKind, Literal,
    LiteralVal, Param, Program, Statement, StructMember, StructTypeId, Type, TypeInterner,
    UnaryExpressionKind,
};

pub trait Irfy {
    type Output;
    fn irfy(self, types: &mut TypeInterner) -> Self::Output;
}

impl Irfy for Program {
    type Output = IRProg;

    fn irfy(self, types: &mut TypeInterner) -> Self::Output {
        IRProg {
            block_items: self
                .block_items
                .into_iter()
                .map(|block_item| block_item.irfy(types))
                .collect(),
        }
    }
}

impl Irfy for BlockItem {
    type Output = IRBlockItem;
    fn irfy(self, types: &mut TypeInterner) -> Self::Output {
        match self {
            BlockItem::Decl(decl) => IRBlockItem::Decl(decl.irfy(types)),
            BlockItem::Stmt(stmt) => IRBlockItem::Stmt(stmt.irfy(types)),
        }
    }
}

impl Irfy for Declaration {
    type Output = IRDecl;

    fn irfy(self, types: &mut TypeInterner) -> Self::Output {
        match self {
            Declaration::Fn {
                name,
                ret,
                params,
                body,
            } => IRDecl::Fn(irfy_fn(name, ret, params, body, types)),
            Declaration::Struct { id, members } => IRDecl::Struct(IRStruct { id, members }),
        }
    }
}

#[derive(Debug)]
struct IRStruct {
    id: StructTypeId,
    members: Vec<StructMember>,
}

fn irfy_fn(
    name: String,
    ret_ty: Option<Type>,
    params: Vec<Param>,
    body: Statement,
    types: &mut TypeInterner,
) -> IRFunc {
    let mut f = IRFunc {
        name,
        params,
        body: vec![],
        ret_ty,
    };

    if let Statement::Block { body } = body {
        for stmt in body {
            f.body.extend(stmt.irfy(types).body);
        }
    }

    if ret_ty.is_none() {
        f.body.push(IRInstr::Ret {
            val: None,
            ret_ty: None,
        });
    }

    f
}

impl Irfy for Statement {
    type Output = IRStmt;

    fn irfy(self, types: &mut TypeInterner) -> Self::Output {
        let mut instrs = vec![];

        match self {
            Statement::Let { name, init, ty } => match ty.unwrap() {
                ty if ty.is_aggregate() => {
                    let dst = IRVal::Var { name, ty };
                    irfy_compound_init(&mut instrs, &dst, 0, &init, types);
                }

                ty => {
                    let src = irfy_expr_and_cvt(&mut instrs, &init, types);

                    let dst = IRVal::Var { name, ty };

                    instrs.push(IRInstr::Cpy {
                        src,
                        src_ty: ty,
                        dst,
                    });
                }
            },

            Statement::Ret { value, ret_ty } => {
                if let Some(value) = &value {
                    let r = irfy_expr_and_cvt(&mut instrs, value, types);

                    instrs.push(IRInstr::Ret {
                        val: Some(r),
                        ret_ty,
                    });
                } else {
                    instrs.push(IRInstr::Ret { val: None, ret_ty });
                }
            }

            Statement::Print { value } => {
                let ty = value.ty().unwrap();
                let r = irfy_expr_and_cvt(&mut instrs, &value, types);

                instrs.push(IRInstr::Print { val: r, ty });
            }

            Statement::If {
                cond,
                then_block,
                else_block,
            } => {
                let r = irfy_expr_and_cvt(&mut instrs, &cond, types);

                let tmp = mktmp();

                if else_block.is_none() {
                    instrs.push(IRInstr::Jz {
                        cond: r,
                        target: format!("End.{}", tmp),
                    });

                    instrs.extend(then_block.irfy(types).body);

                    instrs.push(IRInstr::Label(format!("End.{}", tmp)));
                } else {
                    instrs.push(IRInstr::Jz {
                        cond: r,
                        target: format!("Else.{}", tmp),
                    });
                    instrs.extend(then_block.irfy(types).body);

                    instrs.push(IRInstr::Jmp {
                        target: format!("End.{}", tmp),
                    });

                    instrs.push(IRInstr::Label(format!("Else.{}", tmp)));

                    if let Some(else_block) = else_block {
                        instrs.extend(else_block.irfy(types).body);
                    }

                    instrs.push(IRInstr::Label(format!("End.{}", tmp)));
                }
            }

            Statement::Block { body } => {
                for stmt in body {
                    instrs.extend(stmt.irfy(types).body);
                }
            }

            Statement::While { cond, body, label } => {
                let label = label.as_ref().unwrap();

                let break_label = format!("{label}.break");
                let continue_label = format!("{label}.continue");

                instrs.push(IRInstr::Label(continue_label.clone()));

                /* eval the cond at the start of each iteration  */
                let cond = irfy_expr_and_cvt(&mut instrs, &cond, types);

                /* exit the loop if the cond is false  */
                instrs.push(IRInstr::Jz {
                    cond,
                    target: break_label.clone(),
                });

                /* exec the loop body  */
                instrs.extend(body.irfy(types).body);

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
                instrs.extend(body.irfy(types).body);

                /* continue stmts jmp here to eval the cond  */
                instrs.push(IRInstr::Label(continue_label.clone()));

                /* eval the cond  */
                let cond = irfy_expr_and_cvt(&mut instrs, &cond, types);

                /* exit the loop if the cond is false  */
                instrs.push(IRInstr::Jz {
                    cond,
                    target: break_label.clone(),
                });

                /* start the next iteration  */
                instrs.push(IRInstr::Jmp {
                    target: start_label,
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
                    instrs.extend(init.irfy(types).body);
                }

                /* eval the cond at the start of each iteration  */
                instrs.push(IRInstr::Label(start_label.clone()));

                if let Some(cond) = cond {
                    let cond = irfy_expr_and_cvt(&mut instrs, &cond, types);

                    instrs.push(IRInstr::Jz {
                        cond,
                        target: break_label.clone(),
                    });
                }

                /* exec the loop body  */
                instrs.extend(body.irfy(types).body);

                /* continue stmts jmp here, before the advancement  */
                instrs.push(IRInstr::Label(continue_label));

                /* exec the advancement  */
                if let Some(advancement) = advancement {
                    irfy_expr(&mut instrs, &advancement, types);
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
                instrs.extend(body.irfy(types).body);

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
                instrs.extend(stmt.irfy(types).body);
            }

            Statement::Expr { expr } => {
                irfy_expr(&mut instrs, &expr, types);
            }
        }

        IRStmt { body: instrs }
    }
}

fn irfy_expr_and_cvt(
    instrs: &mut Vec<IRInstr>,
    expr: &Expression,
    types: &mut TypeInterner,
) -> IRVal {
    let r = irfy_expr(instrs, expr, types);

    match r {
        ExpRes::Plain(val) => val,
        ExpRes::DereferencedPtr(ptr) => {
            let dst = mkirvar(expr.ty().unwrap());

            instrs.push(IRInstr::Load {
                src_ptr: ptr,
                dst: dst.clone(),
            });

            dst
        }
        ExpRes::SubObject { base, offset } => {
            let dst = mkirvar(expr.ty().unwrap());

            instrs.push(IRInstr::CpyFromOffset {
                src: base,
                offset,
                dst: dst.clone(),
            });

            dst
        }
    }
}

fn irfy_expr(instrs: &mut Vec<IRInstr>, expr: &Expression, types: &mut TypeInterner) -> ExpRes {
    match expr {
        Expression::Literal {
            val: LiteralVal::String(s),
            ..
        } => ExpRes::Plain(IRVal::Str(s.to_owned())),

        Expression::Literal { val, ty, .. } => {
            let ty = ty.unwrap();
            let literal = Literal::try_from((val, ty)).unwrap();

            ExpRes::Plain(IRVal::Const(literal))
        }

        Expression::Variable { name, ty, .. } => ExpRes::Plain(IRVal::Var {
            name: name.into(),
            ty: ty.unwrap(),
        }),

        Expression::Unary {
            kind: UnaryExpressionKind::AddrOf,
            expr,
            ty,
            ..
        } => {
            let ptr_ty = ty.unwrap();
            match irfy_expr(instrs, expr, types) {
                ExpRes::Plain(var @ IRVal::Var { .. }) => {
                    let dst = mkirvar(ptr_ty);
                    instrs.push(IRInstr::GetAddr {
                        src: var,
                        dst: dst.clone(),
                    });
                    ExpRes::Plain(dst)
                }
                ExpRes::DereferencedPtr(ptr) => ExpRes::Plain(ptr),

                ExpRes::SubObject { base, offset } => {
                    let base_ptr = mkirvar(types.intern_ptr(base.ty()));

                    instrs.push(IRInstr::GetAddr {
                        src: base,
                        dst: base_ptr.clone(),
                    });

                    let dst = mkirvar(ptr_ty);

                    instrs.push(IRInstr::AddPtr {
                        ptr: base_ptr,
                        index: IRVal::Const(Literal::Int64(offset.try_into().unwrap())),
                        scale: 1,
                        dst: dst.clone(),
                    });

                    ExpRes::Plain(dst)
                }

                _ => unreachable!(),
            }
        }

        Expression::Unary {
            kind: UnaryExpressionKind::Deref,
            expr,
            ..
        } => {
            let ptr = irfy_expr_and_cvt(instrs, expr, types);
            ExpRes::DereferencedPtr(ptr)
        }

        Expression::Unary { kind, expr, ty, .. } => {
            let operand_ty = ty.unwrap();

            let src = irfy_expr_and_cvt(instrs, expr, types);

            let dst = mkirvar(operand_ty);

            instrs.push(IRInstr::Unary {
                kind: *kind,
                src,
                dst: dst.clone(),
                operand_ty,
            });

            ExpRes::Plain(dst)
        }

        Expression::Binary {
            kind: BinaryExpressionKind::And,
            lhs,
            rhs,
            ..
        } => {
            let tmp = mktmp();

            let false_label = format!("And.{tmp}.shortcircuit");
            let end_label = format!("And.{tmp}.end");

            let result = mkirvar(Type::Boolean);

            let lhs = irfy_expr_and_cvt(instrs, lhs, types);
            instrs.push(IRInstr::Jz {
                cond: lhs.clone(),
                target: false_label.clone(),
            });

            let rhs = irfy_expr_and_cvt(instrs, rhs, types);
            instrs.push(IRInstr::Jz {
                cond: rhs.clone(),
                target: false_label.clone(),
            });

            instrs.push(IRInstr::Cpy {
                src: IRVal::Const(Literal::Bool(true)),
                src_ty: Type::Boolean,
                dst: result.clone(),
            });

            instrs.push(IRInstr::Jmp {
                target: end_label.clone(),
            });

            instrs.push(IRInstr::Label(false_label.clone()));

            instrs.push(IRInstr::Cpy {
                src: IRVal::Const(Literal::Bool(false)),
                src_ty: Type::Boolean,
                dst: result.clone(),
            });

            instrs.push(IRInstr::Label(end_label));

            ExpRes::Plain(result)
        }

        Expression::Binary {
            kind: BinaryExpressionKind::Or,
            lhs,
            rhs,
            ..
        } => {
            let tmp = mktmp();

            let true_label = format!("Or.{tmp}.shortcircuit");
            let end_label = format!("Or.{tmp}.end");

            let result = mkirvar(Type::Boolean);
            instrs.push(IRInstr::Cpy {
                src: IRVal::Const(Literal::Bool(false)),
                src_ty: Type::Boolean,
                dst: result.clone(),
            });

            let lhs = irfy_expr_and_cvt(instrs, lhs, types);
            instrs.push(IRInstr::Jz {
                cond: lhs.clone(),
                target: true_label.clone(),
            });

            instrs.push(IRInstr::Cpy {
                src: IRVal::Const(Literal::Bool(true)),
                src_ty: Type::Boolean,
                dst: result.clone(),
            });

            instrs.push(IRInstr::Jmp {
                target: end_label.clone(),
            });

            instrs.push(IRInstr::Label(true_label));

            let rhs = irfy_expr_and_cvt(instrs, rhs, types);
            instrs.push(IRInstr::Jz {
                cond: rhs.clone(),
                target: end_label.clone(),
            });

            instrs.push(IRInstr::Cpy {
                src: IRVal::Const(Literal::Bool(true)),
                src_ty: Type::Boolean,
                dst: result.clone(),
            });

            instrs.push(IRInstr::Label(end_label));

            ExpRes::Plain(result)
        }

        Expression::Binary {
            kind, lhs, rhs, ty, ..
        } => {
            let operand_ty = lhs.ty().unwrap();
            let result_ty = ty.unwrap();

            let lhs = irfy_expr_and_cvt(instrs, lhs, types);
            let rhs = irfy_expr_and_cvt(instrs, rhs, types);

            let dst = mkirvar(result_ty);

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
            target, args, ty, ..
        } => {
            let arg_tys = args.iter().map(|arg| arg.ty().unwrap()).collect();

            let ir_args = args
                .iter()
                .map(|arg| irfy_expr_and_cvt(instrs, arg, types))
                .collect();

            let dst = mkirvar(ty.unwrap_or(Type::Int32));

            instrs.push(IRInstr::Call {
                target: target.to_owned(),
                args: ir_args,
                arg_tys,
                dst: dst.clone(),
            });

            ExpRes::Plain(dst)
        }

        Expression::Assign {
            kind, lhs, rhs, ty, ..
        } => {
            /* Force lvalue-to-rvalue conversion for rhs.  */
            let mut rhs = irfy_expr_and_cvt(instrs, rhs, types);

            if *kind != BinaryExpressionKind::Equal {
                let lhs_op = irfy_expr_and_cvt(instrs, lhs, types);
                let tmp = mkirvar(ty.unwrap());
                instrs.push(IRInstr::Bin {
                    kind: *kind,
                    lhs: lhs_op.clone(),
                    rhs: rhs.clone(),
                    dst: tmp.clone(),
                    operand_ty: ty.clone().unwrap(),
                });

                rhs = tmp;
            }

            /* Keep lhs as lvalue.  */
            let lhs_op = irfy_expr(instrs, lhs, types);

            match lhs_op {
                ExpRes::Plain(op) => {
                    instrs.push(IRInstr::Cpy {
                        src: rhs.clone(),
                        src_ty: ty.unwrap(),
                        dst: op,
                    });
                }
                ExpRes::DereferencedPtr(ptr) => {
                    instrs.push(IRInstr::Store {
                        src: rhs.clone(),
                        dst_ptr: ptr,
                    });
                }
                ExpRes::SubObject { base, offset } => {
                    instrs.push(IRInstr::CpyToOffset {
                        src: rhs.clone(),
                        dst: base,
                        offset,
                    });
                }
            }

            /* Assignments evaluate to their rhs val.  */
            ExpRes::Plain(rhs)
        }

        Expression::Cast {
            expr, target_ty, ..
        } => {
            let src_ty = expr.ty().unwrap();

            let src = irfy_expr_and_cvt(instrs, expr, types);

            let dst = irfy_cast(instrs, src, src_ty, *target_ty, types);

            ExpRes::Plain(dst)
        }

        Expression::Index {
            array, index, ty, ..
        } => {
            let element_ty = ty.unwrap();

            /* Evaluate the array expression and obtain a pointer.  */
            let ptr = irfy_array_base(instrs, array, types);

            /* Evaluate the index expression.  */
            let index = irfy_expr_and_cvt(instrs, index, types);

            /* Determine the element size in bytes.  */
            let scale = element_ty.size_of(types);

            /* Create a pointer to the element.  */
            let dst_ty = types.intern_ptr(element_ty);
            let dst = mkirvar(dst_ty);

            /* Calculate the address.  */
            instrs.push(IRInstr::AddPtr {
                ptr,
                index,
                scale,
                dst: dst.clone(),
            });

            ExpRes::DereferencedPtr(dst)
        }

        Expression::ArrayLiteral { ty, .. } => {
            let ty = ty.unwrap();

            /* Allocate a temporary aggregate object.  */
            let dst = mkirvar(ty);

            let IRVal::Var { name, .. } = &dst else {
                unreachable!();
            };

            /* Initialize the aggregate recursively.  */
            irfy_compound_init(instrs, &dst, 0, expr, types);

            ExpRes::Plain(dst)
        }

        Expression::Get {
            kind,
            object,
            field,
            ty,
            ..
        } => {
            let object_ty = object.ty().unwrap();
            let field_ty = ty.unwrap();

            let struct_ty = match (*kind, object_ty) {
                (GetExpressionKind::Dot, ty) => ty,
                (GetExpressionKind::Arrow, Type::Ptr(id)) => types.get_pointer(id),
                _ => unreachable!(),
            };

            let Type::Struct(id) = struct_ty else {
                unreachable!()
            };

            let offset = types
                .get_struct_layout(id)
                .fields
                .get(field)
                .unwrap()
                .offset;

            let object = match kind {
                GetExpressionKind::Dot => irfy_expr(instrs, object, types),
                GetExpressionKind::Arrow => {
                    ExpRes::DereferencedPtr(irfy_expr_and_cvt(instrs, object, types))
                }
            };

            match object {
                ExpRes::Plain(base @ IRVal::Var { .. }) => ExpRes::SubObject { base, offset },

                ExpRes::SubObject { base, offset: prev } => ExpRes::SubObject {
                    base,
                    offset: prev + offset,
                },

                ExpRes::DereferencedPtr(ptr) => {
                    let dst = mkirvar(types.intern_ptr(field_ty));

                    instrs.push(IRInstr::AddPtr {
                        ptr,
                        index: IRVal::Const(Literal::Int64(i64::try_from(offset).unwrap())),
                        scale: 1,
                        dst: dst.clone(),
                    });

                    ExpRes::DereferencedPtr(dst)
                }

                _ => unreachable!(),
            }
        }

        Expression::StructLiteral { ty, .. } => {
            let dst = mkirvar(ty.unwrap());

            irfy_compound_init(instrs, &dst, 0, expr, types);

            ExpRes::Plain(dst)
        }
    }
}

fn irfy_cast(
    instrs: &mut Vec<IRInstr>,
    src: IRVal,
    src_ty: Type,
    dst_ty: Type,
    types: &mut TypeInterner,
) -> IRVal {
    /* Casting a val to its own type is a NOP.  */
    if src_ty == dst_ty {
        return src;
    }

    let dst = mkirvar(dst_ty);

    let instr = match (src_ty, dst_ty) {
        /* int -> int  */
        (from, to) if from.is_int() && to.is_int() => {
            use std::cmp::Ordering;

            match from.size_of(types).cmp(&to.size_of(types)) {
                Ordering::Equal => IRInstr::Cpy {
                    src,
                    src_ty,
                    dst: dst.clone(),
                },
                Ordering::Greater => IRInstr::Truncate {
                    src,
                    dst: dst.clone(),
                    src_ty,
                    dst_ty,
                },
                Ordering::Less if from.is_signed() => IRInstr::SignExtend {
                    src,
                    dst: dst.clone(),
                    src_ty,
                    dst_ty,
                },
                Ordering::Less => IRInstr::ZeroExtend {
                    src,
                    dst: dst.clone(),
                    src_ty,
                    dst_ty,
                },
            }
        }

        /* double -> signed int  */
        (Type::Float64, to) if to.is_int() && to.is_signed() => IRInstr::DoubleToInt {
            src,
            dst: dst.clone(),
            src_ty,
            dst_ty,
        },

        /* double -> unsigned int  */
        (Type::Float64, to) if to.is_int() && !to.is_signed() => IRInstr::DoubleToUInt {
            src,
            dst: dst.clone(),
            src_ty,
            dst_ty,
        },

        /* signed int -> double  */
        (from, Type::Float64) if from.is_int() && from.is_signed() => IRInstr::IntToDouble {
            src,
            dst: dst.clone(),
            src_ty,
            dst_ty,
        },

        /* unsigned int -> double  */
        (from, Type::Float64) if from.is_int() && !from.is_signed() => IRInstr::UIntToDouble {
            src,
            dst: dst.clone(),
            src_ty,
            dst_ty,
        },

        /* double -> float  */
        (Type::Float64, Type::Float32) => IRInstr::DoubleToFloat {
            src,
            dst: dst.clone(),
            src_ty,
            dst_ty,
        },

        /* float -> double */
        (Type::Float32, Type::Float64) => IRInstr::FloatToDouble {
            src,
            dst: dst.clone(),
            src_ty,
            dst_ty,
        },

        /* float -> signed int  */
        (Type::Float32, to) if to.is_int() && to.is_signed() => IRInstr::FloatToInt {
            src,
            dst: dst.clone(),
            src_ty,
            dst_ty,
        },

        /* float -> unsigned int  */
        (Type::Float32, to) if to.is_int() && !to.is_signed() => IRInstr::FloatToUInt {
            src,
            dst: dst.clone(),
            src_ty,
            dst_ty,
        },

        /* signed int -> float  */
        (from, Type::Float32) if from.is_int() && from.is_signed() => IRInstr::IntToFloat {
            src,
            dst: dst.clone(),
            src_ty,
            dst_ty,
        },

        /* unsigned int -> float  */
        (from, Type::Float32) if from.is_int() && !from.is_signed() => IRInstr::UIntToFloat {
            src,
            dst: dst.clone(),
            src_ty,
            dst_ty,
        },

        _ => unreachable!("Unsupported cast from {src_ty:?} to {dst_ty:?}"),
    };

    instrs.push(instr);

    dst
}

fn irfy_compound_init(
    instrs: &mut Vec<IRInstr>,
    dst: &IRVal,
    offset: usize,
    init: &Expression,
    types: &mut TypeInterner,
) {
    match init {
        Expression::ArrayLiteral {
            elements,
            ty: Some(Type::Array(id)),
            ..
        } => {
            let array = types.get_array(*id);
            let element_size = array.element.size_of(types);

            for (i, element) in elements.iter().enumerate() {
                let offset = offset + i * element_size;

                irfy_compound_init(instrs, dst, offset, element, types);
            }
        }

        Expression::StructLiteral {
            members,
            ty: Some(Type::Struct(id)),
            ..
        } => {
            for member in members {
                let field_offset = types
                    .get_struct_layout(*id)
                    .fields
                    .get(&member.name)
                    .unwrap()
                    .offset;

                irfy_compound_init(instrs, dst, offset + field_offset, &member.init, types);
            }
        }

        _ => {
            let src = irfy_expr_and_cvt(instrs, init, types);

            instrs.push(IRInstr::CpyToOffset {
                src,
                dst: dst.to_owned(),
                offset,
            });
        }
    }
}

fn irfy_array_base(
    instrs: &mut Vec<IRInstr>,
    array: &Expression,
    types: &mut TypeInterner,
) -> IRVal {
    let array_ty = array.ty().unwrap();

    let Type::Array(id) = array_ty else {
        unreachable!("expected array type");
    };

    let element_ty = types.get_array(id).element;
    let ptr_ty = types.intern_ptr(element_ty);

    let result = irfy_expr(instrs, array, types);

    match result {
        ExpRes::Plain(IRVal::Var { name, ty }) => {
            let dst = mkirvar(ptr_ty);

            instrs.push(IRInstr::GetAddr {
                src: IRVal::Var { name, ty },
                dst: dst.clone(),
            });

            dst
        }

        ExpRes::DereferencedPtr(ptr) => {
            let src_ty = match &ptr {
                IRVal::Var { ty, .. } => *ty,
                _ => unreachable!(),
            };

            if src_ty == ptr_ty {
                ptr
            } else {
                let dst = mkirvar(ptr_ty);

                instrs.push(IRInstr::Cpy {
                    src: ptr,
                    src_ty,
                    dst: dst.clone(),
                });

                dst
            }
        }

        ExpRes::SubObject { base, offset } => {
            let base_ptr = mkirvar(types.intern_ptr(base.ty()));

            instrs.push(IRInstr::GetAddr {
                src: base,
                dst: base_ptr.clone(),
            });

            let dst = mkirvar(ptr_ty);

            instrs.push(IRInstr::AddPtr {
                ptr: base_ptr,
                index: IRVal::Const(Literal::Int64(offset.try_into().unwrap())),
                scale: 1,
                dst: dst.clone(),
            });

            dst
        }

        ExpRes::Plain(_) => unreachable!(),
    }
}

#[derive(Clone)]
pub enum IRInstr {
    Cpy {
        src: IRVal,
        src_ty: Type,
        dst: IRVal,
    },
    Ret {
        val: Option<IRVal>,
        ret_ty: Option<Type>,
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
    SignExtend {
        src: IRVal,
        dst: IRVal,
        src_ty: Type,
        dst_ty: Type,
    },
    ZeroExtend {
        src: IRVal,
        dst: IRVal,
        src_ty: Type,
        dst_ty: Type,
    },
    Truncate {
        src: IRVal,
        dst: IRVal,
        src_ty: Type,
        dst_ty: Type,
    },
    DoubleToInt {
        src: IRVal,
        dst: IRVal,
        src_ty: Type,
        dst_ty: Type,
    },
    IntToDouble {
        src: IRVal,
        dst: IRVal,
        src_ty: Type,
        dst_ty: Type,
    },
    DoubleToUInt {
        src: IRVal,
        src_ty: Type,
        dst: IRVal,
        dst_ty: Type,
    },
    UIntToDouble {
        src: IRVal,
        src_ty: Type,
        dst: IRVal,
        dst_ty: Type,
    },
    DoubleToFloat {
        src: IRVal,
        src_ty: Type,
        dst: IRVal,
        dst_ty: Type,
    },
    FloatToDouble {
        src: IRVal,
        src_ty: Type,
        dst: IRVal,
        dst_ty: Type,
    },
    FloatToInt {
        src: IRVal,
        src_ty: Type,
        dst: IRVal,
        dst_ty: Type,
    },
    IntToFloat {
        src: IRVal,
        src_ty: Type,
        dst: IRVal,
        dst_ty: Type,
    },
    FloatToUInt {
        src: IRVal,
        src_ty: Type,
        dst: IRVal,
        dst_ty: Type,
    },
    UIntToFloat {
        src: IRVal,
        src_ty: Type,
        dst: IRVal,
        dst_ty: Type,
    },
    GetAddr {
        src: IRVal,
        dst: IRVal,
    },
    Load {
        src_ptr: IRVal,
        dst: IRVal,
    },
    Store {
        src: IRVal,
        dst_ptr: IRVal,
    },
    AddPtr {
        ptr: IRVal,
        index: IRVal,
        scale: usize,
        dst: IRVal,
    },
    CpyToOffset {
        src: IRVal,
        dst: IRVal,
        offset: usize,
    },
    CpyFromOffset {
        src: IRVal,
        offset: usize,
        dst: IRVal,
    },
}

impl IRInstr {
    pub fn dst(&self) -> Option<&IRVal> {
        match self {
            Self::Cpy { dst, .. }
            | Self::Bin { dst, .. }
            | Self::Unary { dst, .. }
            | Self::Call { dst, .. }
            | Self::SignExtend { dst, .. }
            | Self::ZeroExtend { dst, .. }
            | Self::Truncate { dst, .. }
            | Self::DoubleToInt { dst, .. }
            | Self::IntToDouble { dst, .. }
            | Self::DoubleToUInt { dst, .. }
            | Self::UIntToDouble { dst, .. }
            | Self::DoubleToFloat { dst, .. }
            | Self::FloatToDouble { dst, .. }
            | Self::FloatToInt { dst, .. }
            | Self::IntToFloat { dst, .. }
            | Self::FloatToUInt { dst, .. }
            | Self::UIntToFloat { dst, .. }
            | Self::GetAddr { dst, .. }
            | Self::Load { dst, .. }
            | Self::AddPtr { dst, .. }
            | Self::CpyFromOffset { dst, .. } => Some(dst),

            _ => None,
        }
    }
}

impl IRVal {
    pub fn ty(&self) -> Type {
        match self {
            IRVal::Str(_) => Type::String,
            IRVal::Var { ty, .. } => *ty,
            IRVal::Const(v) => Type::from(*v),
        }
    }
}

impl std::fmt::Debug for IRInstr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cpy { src, dst, .. } => {
                write!(f, "{dst} = {src}")
            }

            Self::Ret { val, .. } => match val {
                Some(val) => write!(f, "return {val}"),
                None => write!(f, "return"),
            },

            Self::Bin {
                kind,
                lhs,
                rhs,
                dst,
                ..
            } => {
                let op = match kind {
                    BinaryExpressionKind::Add => "+",
                    BinaryExpressionKind::Sub => "-",
                    BinaryExpressionKind::Mul => "*",
                    BinaryExpressionKind::Div => "/",
                    BinaryExpressionKind::Rem => "%",
                    BinaryExpressionKind::Shl => "<<",
                    BinaryExpressionKind::Shr => ">>",
                    BinaryExpressionKind::BitwiseAnd => "&",
                    BinaryExpressionKind::BitwiseOr => "|",
                    BinaryExpressionKind::BitwiseXor => "^",
                    BinaryExpressionKind::And => "&&",
                    BinaryExpressionKind::Or => "||",
                    BinaryExpressionKind::Less => "<",
                    BinaryExpressionKind::Greater => ">",
                    BinaryExpressionKind::LessEqual => "<=",
                    BinaryExpressionKind::GreaterEqual => ">=",
                    BinaryExpressionKind::Equal => "==",
                    BinaryExpressionKind::NotEqual => "!=",
                };

                write!(f, "{dst} = {lhs} {op} {rhs}")
            }

            Self::Unary { kind, src, dst, .. } => {
                let op = match kind {
                    UnaryExpressionKind::Negate => "-",
                    UnaryExpressionKind::LogicalNot => "!",
                    UnaryExpressionKind::BitwiseNot => "~",
                    UnaryExpressionKind::Deref => "*",
                    UnaryExpressionKind::AddrOf => "&",
                };

                write!(f, "{dst} = {op}{src}")
            }

            Self::Print { val, .. } => {
                write!(f, "print {val}")
            }

            Self::Jmp { target } => {
                write!(f, "jmp {target}")
            }

            Self::Jz { cond, target } => {
                write!(f, "jz {cond}, {target}")
            }

            Self::Label(name) => {
                write!(f, "{name}:")
            }

            Self::Call {
                target, args, dst, ..
            } => {
                write!(f, "{dst} = call {target}(")?;

                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }

                    write!(f, "{arg}")?;
                }

                write!(f, ")")
            }

            Self::SignExtend {
                src,
                dst,
                src_ty,
                dst_ty,
            } => fmt_conversion(f, "sext", src, dst, src_ty, dst_ty),

            Self::ZeroExtend {
                src,
                dst,
                src_ty,
                dst_ty,
            } => fmt_conversion(f, "zext", src, dst, src_ty, dst_ty),

            Self::Truncate {
                src,
                dst,
                src_ty,
                dst_ty,
            } => fmt_conversion(f, "trunc", src, dst, src_ty, dst_ty),

            Self::DoubleToInt {
                src,
                dst,
                src_ty,
                dst_ty,
            } => fmt_conversion(f, "double_to_int", src, dst, src_ty, dst_ty),

            Self::IntToDouble {
                src,
                dst,
                src_ty,
                dst_ty,
            } => fmt_conversion(f, "int_to_double", src, dst, src_ty, dst_ty),

            Self::DoubleToUInt {
                src,
                dst,
                src_ty,
                dst_ty,
            } => fmt_conversion(f, "double_to_uint", src, dst, src_ty, dst_ty),

            Self::UIntToDouble {
                src,
                dst,
                src_ty,
                dst_ty,
            } => fmt_conversion(f, "uint_to_double", src, dst, src_ty, dst_ty),

            Self::DoubleToFloat {
                src,
                dst,
                src_ty,
                dst_ty,
            } => fmt_conversion(f, "double_to_float", src, dst, src_ty, dst_ty),

            Self::FloatToDouble {
                src,
                dst,
                src_ty,
                dst_ty,
            } => fmt_conversion(f, "float_to_double", src, dst, src_ty, dst_ty),

            Self::FloatToInt {
                src,
                dst,
                src_ty,
                dst_ty,
            } => fmt_conversion(f, "float_to_int", src, dst, src_ty, dst_ty),

            Self::IntToFloat {
                src,
                dst,
                src_ty,
                dst_ty,
            } => fmt_conversion(f, "int_to_float", src, dst, src_ty, dst_ty),

            Self::FloatToUInt {
                src,
                dst,
                src_ty,
                dst_ty,
            } => fmt_conversion(f, "float_to_uint", src, dst, src_ty, dst_ty),

            Self::UIntToFloat {
                src,
                dst,
                src_ty,
                dst_ty,
            } => fmt_conversion(f, "uint_to_float", src, dst, src_ty, dst_ty),

            Self::GetAddr { src, dst } => {
                write!(f, "{dst} = &{src}")
            }

            Self::Load { src_ptr, dst } => {
                write!(f, "{dst} = *{src_ptr}")
            }

            Self::Store { src, dst_ptr } => {
                write!(f, "*{dst_ptr} = {src}")
            }

            Self::AddPtr {
                ptr,
                index,
                scale,
                dst,
            } => {
                write!(f, "{dst} = {ptr} + {index} * {scale}")
            }

            Self::CpyToOffset { src, dst, offset } => {
                write!(f, "{dst}[byte {offset}] = {src}")
            }

            Self::CpyFromOffset { src, offset, dst } => {
                write!(f, "{dst} = {src}[byte {offset}]")
            }
        }
    }
}

fn fmt_conversion(
    f: &mut std::fmt::Formatter<'_>,
    name: &str,
    src: &IRVal,
    dst: &IRVal,
    src_ty: &Type,
    dst_ty: &Type,
) -> std::fmt::Result {
    write!(f, "{dst} = {name}({src}: {src_ty:?} -> {dst_ty:?})")
}

#[derive(Debug)]
pub enum IRBlockItem {
    Decl(IRDecl),
    Stmt(IRStmt),
}

#[derive(Debug)]
pub struct IRProg {
    pub block_items: Vec<IRBlockItem>,
}

impl IRProg {
    pub fn functions(&self) -> impl Iterator<Item = &IRFunc> {
        self.block_items.iter().filter_map(|item| match item {
            IRBlockItem::Decl(IRDecl::Fn(func)) => Some(func),
            _ => None,
        })
    }

    pub fn functions_mut(&mut self) -> impl Iterator<Item = &mut IRFunc> {
        self.block_items.iter_mut().filter_map(|item| match item {
            IRBlockItem::Decl(IRDecl::Fn(func)) => Some(func),
            _ => None,
        })
    }

    pub fn function(&self, index: usize) -> Option<&IRFunc> {
        self.functions().nth(index)
    }
}

#[derive(Debug, Clone)]
pub struct IRFunc {
    pub name: String,
    pub params: Vec<Param>,
    pub body: Vec<IRInstr>,
    pub ret_ty: Option<Type>,
}

#[derive(Debug, Clone)]
pub enum StaticInit {
    Float(f32),
    Double(f64),
    String(String),
}

#[derive(Debug)]
pub enum IRDecl {
    Fn(IRFunc),
    Struct(IRStruct),
}

#[derive(Debug)]
pub struct IRStmt {
    pub body: Vec<IRInstr>,
}

#[derive(Debug, Clone)]
pub enum IRVal {
    Const(Literal),
    Str(String),
    Var { name: String, ty: Type },
}

impl std::fmt::Display for IRVal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Var { name, .. } => write!(f, "{name}"),

            Self::Const(literal) => match literal {
                Literal::Bool(v) => write!(f, "{v}"),
                Literal::UInt8(v) => write!(f, "{v}"),
                Literal::Int8(v) => write!(f, "{v}"),
                Literal::UInt16(v) => write!(f, "{v}"),
                Literal::Int16(v) => write!(f, "{v}"),
                Literal::UInt32(v) => write!(f, "{v}"),
                Literal::Int32(v) => write!(f, "{v}"),
                Literal::UInt64(v) => write!(f, "{v}"),
                Literal::Int64(v) => write!(f, "{v}"),
                Literal::Float32(v) => write!(f, "{v}"),
                Literal::Float64(v) => write!(f, "{v}"),
            },

            Self::Str(string) => write!(f, "{string}"),
        }
    }
}

enum ExpRes {
    Plain(IRVal),
    DereferencedPtr(IRVal),
    SubObject { base: IRVal, offset: usize },
}

fn mkirvar(ty: Type) -> IRVal {
    let tmp = mktmp();

    IRVal::Var {
        name: format!("tmp.{}", tmp),
        ty,
    }
}

pub fn mktmp() -> u64 {
    static NEXT_TMP: AtomicU64 = AtomicU64::new(0);

    NEXT_TMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
