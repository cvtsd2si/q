use crate::{
    ir::{
        cfg::Cfg,
        r#gen::{IRFunc, IRInstr, IRProg, IRVal},
        liveness::Liveness,
    },
    parser::ast::{BinaryExpressionKind, Literal, Type, UnaryExpressionKind},
    runtime::value::{Value, ValueTag},
};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub enum Optimization {
    ConstantFolding,
    ConstantPropagation,
    CopyPropagation,
    DeadStoreElimination,
    UnreachableCodeElimination,
}

pub fn optimize_ir(mut ir: IRProg, enabled: &[Optimization]) -> IRProg {
    for func in &mut ir.funcs {
        loop {
            let mut changed = false;

            for pass in enabled {
                changed |= match pass {
                    Optimization::ConstantFolding => constant_folding(func),
                    Optimization::ConstantPropagation => constant_propagation(func),
                    Optimization::CopyPropagation => copy_propagation(func),
                    Optimization::UnreachableCodeElimination => eliminate_unreachable(func),
                    Optimization::DeadStoreElimination => eliminate_dead_stores(func),
                };
            }

            if !changed {
                break;
            }
        }
    }

    ir
}

fn constant_folding(func: &mut IRFunc) -> bool {
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

    let result: Value = match kind {
        BinaryExpressionKind::Add if is_numeric => av + bv,
        BinaryExpressionKind::Sub if is_numeric => av - bv,
        BinaryExpressionKind::Mul if is_numeric => av * bv,
        BinaryExpressionKind::Div if is_numeric => av / bv,
        BinaryExpressionKind::Rem if is_int => av % bv,
        BinaryExpressionKind::Shl if is_int => av << bv,
        BinaryExpressionKind::Shr if is_int => av >> bv,
        BinaryExpressionKind::BitwiseAnd if is_int || is_bool => av & bv,
        BinaryExpressionKind::BitwiseOr if is_int || is_bool => av | bv,
        BinaryExpressionKind::BitwiseXor if is_int || is_bool => av ^ bv,
        BinaryExpressionKind::And if is_bool => av & bv,
        BinaryExpressionKind::Or if is_bool => av | bv,
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

impl From<Value> for Literal {
    fn from(value: Value) -> Self {
        match value.tag {
            ValueTag::Bool => Literal::Bool(value.payload != 0),
            ValueTag::UInt8 => Literal::UInt8(value.payload as u8),
            ValueTag::Int8 => Literal::Int8(value.payload as i8),
            ValueTag::UInt16 => Literal::UInt16(value.payload as u16),
            ValueTag::Int16 => Literal::Int16(value.payload as i16),
            ValueTag::UInt32 => Literal::UInt32(value.payload as u32),
            ValueTag::Int32 => Literal::Int32(value.payload as i32),
            ValueTag::UInt64 => Literal::UInt64(value.payload),
            ValueTag::Int64 => Literal::Int64(value.payload as i64),
            ValueTag::Float32 => Literal::Float32(f32::from_bits(value.payload as u32)),
            ValueTag::Float64 => Literal::Float64(f64::from_bits(value.payload)),
            _ => unreachable!(),
        }
    }
}

impl Into<Type> for Literal {
    fn into(self) -> Type {
        match self {
            Literal::Bool(_) => Type::Boolean,
            Literal::Int8(_) => Type::Int8,
            Literal::UInt8(_) => Type::UInt8,
            Literal::Int16(_) => Type::Int16,
            Literal::UInt16(_) => Type::UInt16,
            Literal::Int32(_) => Type::Int32,
            Literal::UInt32(_) => Type::UInt32,
            Literal::Int64(_) => Type::Int64,
            Literal::UInt64(_) => Type::UInt64,
            Literal::Float32(_) => Type::Float32,
            Literal::Float64(_) => Type::Float64,
        }
    }
}

fn constant_propagation(func: &mut IRFunc) -> bool {
    let cfg = Cfg::build(func).unwrap();
    let n = cfg.blocks.len();

    /* Assume none of the blocks are reachable at first.  */
    let mut reachable_blocks = vec![false; n];

    let mut stack = vec![0usize];

    while let Some(block_idx) = stack.pop() {
        /* If we have visited this block before, continue.  */
        if reachable_blocks[block_idx] {
            continue;
        }

        /* Mark the block as reachable.  */
        reachable_blocks[block_idx] = true;

        /* Push the block's successors onto the stack.  */
        stack.extend(cfg.blocks[block_idx].succs.iter().map(|id| id.0));
    }

    /* For each basic block, create a HashMap holding information about
     * which variables are known to be consant after executing that block.  */
    let mut out = vec![HashMap::new(); n];

    loop {
        let mut analysis_changed = false;

        for (idx, block) in cfg.blocks.iter().enumerate() {
            /* Skip unreachable blocks.  */
            if !reachable_blocks[idx] {
                continue;
            }

            let mut facts = incoming_constants(idx, &cfg, &out, &reachable_blocks);
            for i in block.range.clone() {
                transfer_constant(&func.body[i], &mut facts);
            }

            if !same_constants(&out[idx], &facts) {
                out[idx] = facts;
                analysis_changed = true;
            }
        }

        if !analysis_changed {
            break;
        }
    }

    /* Only rewrite after the analysis has converged.  */
    let mut changed = false;
    for (idx, block) in cfg.blocks.iter().enumerate() {
        /* Skip unreachable blocks.  */
        if !reachable_blocks[idx] {
            continue;
        }

        let mut facts = incoming_constants(idx, &cfg, &out, &reachable_blocks);

        for i in block.range.clone() {
            let instr = &mut func.body[i];

            visit_reads(instr, |val| {
                if let IRVal::Var { name, ty } = val {
                    if let Some(&konst) = facts.get(name) {
                        /* Never replace a typed read with a different type.  */
                        if Into::<Type>::into(konst) == *ty {
                            *val = IRVal::Const(konst);
                            changed = true;
                        }
                    }
                }
            });

            /* Track the state after this instruction (including any rewrite).  */
            transfer_constant(instr, &mut facts);
        }
    }

    changed
}

fn incoming_constants(
    block: usize,
    cfg: &Cfg,
    out: &[HashMap<String, Literal>],
    reachable_blocks: &[bool],
) -> HashMap<String, Literal> {
    /* Entry is a boundary: all parameters are unknown,
     * even if it has incoming backedges.  */
    if block == 0 {
        return HashMap::new();
    }

    /* Find the block's reachable predecessors.  */
    let mut preds = cfg.blocks[block]
        .preds
        .iter()
        .map(|id| id.0)
        .filter(|&id| reachable_blocks[id]);

    /* If there are no reachable predecessors, there is no
     * incoming constant information.
     *
     * Otherwise, grab the first predecessor.  */
    let Some(first) = preds.next() else {
        return HashMap::new();
    };

    /* We start with what's known at the first predecessor.  */
    let mut facts = out[first].clone();

    /* Intersect what's known at the first predecessor with each
     * subsequent predecessor, and only keep variables whose values agree.  */
    for p in preds {
        facts.retain(|name, value| {
            out[p]
                .get(name)
                .is_some_and(|other| same_constant(*value, *other))
        });
    }

    facts
}

fn transfer_constant(instr: &IRInstr, facts: &mut HashMap<String, Literal>) {
    /* Does this instruction assign a value to a variable?
     * If yes, extract the dst and dst_ty.
     * If not, return.  */
    let Some(IRVal::Var {
        name: dst,
        ty: dst_ty,
    }) = instr.dst()
    else {
        return;
    };

    /* Determine whether the assigned value is constant.  */
    let new_const = match instr {
        IRInstr::Cpy { src, src_ty, .. } if src_ty == dst_ty => match src {
            IRVal::Const(value) => Some(*value),
            IRVal::Var { name, .. } => facts.get(name).copied(),
        },
        _ => None,
    };

    /* The instruction overwrites the variable, so its previously known
     * constant might no longer be valid.  */
    facts.remove(dst);

    if let Some(value) = new_const {
        facts.insert(dst.clone(), value);
    }
}

fn same_constant(a: Literal, b: Literal) -> bool {
    let a = Value::from(a);
    let b = Value::from(b);
    a.tag == b.tag && a.payload == b.payload
}

fn same_constants(a: &HashMap<String, Literal>, b: &HashMap<String, Literal>) -> bool {
    a.len() == b.len()
        && a.iter().all(|(name, value)| {
            b.get(name)
                .is_some_and(|other| same_constant(*value, *other))
        })
}

fn visit_reads(instr: &mut IRInstr, mut visit: impl FnMut(&mut IRVal)) {
    match instr {
        IRInstr::Cpy { src, .. }
        | IRInstr::Unary { src, .. }
        | IRInstr::SignExtend { src, .. }
        | IRInstr::ZeroExtend { src, .. }
        | IRInstr::Truncate { src, .. }
        | IRInstr::DoubleToInt { src, .. }
        | IRInstr::IntToDouble { src, .. }
        | IRInstr::DoubleToUInt { src, .. }
        | IRInstr::UIntToDouble { src, .. }
        | IRInstr::DoubleToFloat { src, .. }
        | IRInstr::FloatToDouble { src, .. }
        | IRInstr::FloatToInt { src, .. }
        | IRInstr::IntToFloat { src, .. }
        | IRInstr::FloatToUInt { src, .. }
        | IRInstr::UIntToFloat { src, .. } => visit(src),

        IRInstr::Bin { lhs, rhs, .. } => {
            visit(lhs);
            visit(rhs);
        }
        IRInstr::Ret { val: Some(val), .. }
        | IRInstr::Print { val, .. }
        | IRInstr::Jz { cond: val, .. } => visit(val),
        IRInstr::Call { args, .. } => {
            for arg in args {
                visit(arg);
            }
        }
        IRInstr::Ret { val: None, .. } | IRInstr::Label(_) | IRInstr::Jmp { .. } => {}
    }
}

fn eliminate_dead_stores(func: &mut IRFunc) -> bool {
    let cfg = Cfg::build(func).unwrap();
    let liveness = Liveness::analyze(func, &cfg);

    let original_len = func.body.len();
    let mut idx = 0;

    func.body.retain(|instr| {
        let live_out = &liveness.instrs[idx].live_out;
        idx += 1;

        // Check whether the instruction defines a variable
        // that is not live afterward.
        let dead = match instr.dst() {
            Some(IRVal::Var { name, .. }) => !live_out.contains(name),

            _ => false,
        };

        // Remove only dead assignments without side effects.
        !(dead && is_pure(instr))
    });

    func.body.len() != original_len
}

fn is_pure(instr: &IRInstr) -> bool {
    match instr {
        IRInstr::Cpy { .. }
        | IRInstr::Unary { .. }
        | IRInstr::SignExtend { .. }
        | IRInstr::ZeroExtend { .. }
        | IRInstr::Truncate { .. }
        | IRInstr::DoubleToInt { .. }
        | IRInstr::IntToDouble { .. }
        | IRInstr::DoubleToUInt { .. }
        | IRInstr::UIntToDouble { .. }
        | IRInstr::DoubleToFloat { .. }
        | IRInstr::FloatToDouble { .. }
        | IRInstr::FloatToInt { .. }
        | IRInstr::IntToFloat { .. }
        | IRInstr::FloatToUInt { .. }
        | IRInstr::UIntToFloat { .. } => true,

        IRInstr::Bin { kind, .. } => {
            !matches!(kind, BinaryExpressionKind::Div | BinaryExpressionKind::Rem)
        }

        _ => false,
    }
}

fn eliminate_unreachable(func: &mut IRFunc) -> bool {
    let cfg = Cfg::build(func).unwrap();
    let n = cfg.blocks.len();

    if n == 0 {
        return false;
    }

    /* Initially, no blocks are considered reachable.  */
    let mut reachable = vec![false; n];

    /* The entry block is always block 0.  */
    let mut stack = vec![0usize];

    /* Find all blocks reachable from the entry.  */
    while let Some(block_idx) = stack.pop() {
        if reachable[block_idx] {
            continue;
        }

        reachable[block_idx] = true;

        /* Every successor of a reachable block is also reachable.  */
        for successor in &cfg.blocks[block_idx].succs {
            stack.push(successor.0);
        }
    }

    /* Remove instructions belonging to unreachable blocks.  */
    let original_len = func.body.len();
    let mut instr_idx = 0;

    func.body.retain(|_| {
        let block_idx = cfg.instr_to_block[instr_idx].0;
        instr_idx += 1;

        reachable[block_idx]
    });

    func.body.len() != original_len
}

type CopyFacts = HashMap<String, (String, Type)>;

fn copy_propagation(func: &mut IRFunc) -> bool {
    let cfg = Cfg::build(func).unwrap();
    let n = cfg.blocks.len();

    if n == 0 {
        return false;
    }

    let mut reachable = vec![false; n];
    let mut stack = vec![0usize];

    while let Some(block_idx) = stack.pop() {
        if reachable[block_idx] {
            continue;
        }

        reachable[block_idx] = true;

        for successor in &cfg.blocks[block_idx].succs {
            stack.push(successor.0);
        }
    }

    let mut out: Vec<CopyFacts> = vec![HashMap::new(); n];

    loop {
        let mut analysis_changed = false;

        for (idx, block) in cfg.blocks.iter().enumerate() {
            if !reachable[idx] {
                continue;
            }

            let mut facts = incoming_copies(idx, &cfg, &out, &reachable);

            /* Simulate each instruction to determine
             * which copies are valid at the block exit.  */
            for i in block.range.clone() {
                transfer_copy(&func.body[i], &mut facts);
            }

            if out[idx] != facts {
                out[idx] = facts;
                analysis_changed = true;
            }
        }

        if !analysis_changed {
            break;
        }
    }

    let mut changed = false;

    for (idx, block) in cfg.blocks.iter().enumerate() {
        if !reachable[idx] {
            continue;
        }

        let mut facts = incoming_copies(idx, &cfg, &out, &reachable);

        for i in block.range.clone() {
            let instr = &mut func.body[i];

            visit_reads(instr, |val| {
                let replacement = match val {
                    IRVal::Var { name, ty } => {
                        facts.get(name.as_str()).and_then(|(source, source_ty)| {
                            if *ty == *source_ty && name.as_str() != source.as_str() {
                                Some(IRVal::Var {
                                    name: source.clone(),
                                    ty: *source_ty,
                                })
                            } else {
                                None
                            }
                        })
                    }

                    IRVal::Const(_) => None,
                };

                if let Some(replacement) = replacement {
                    *val = replacement;
                    changed = true;
                }
            });

            /* Update copy information after the instruction,
             * accounting for the rewritten operands.  */
            transfer_copy(instr, &mut facts);
        }
    }

    changed
}

fn incoming_copies(block: usize, cfg: &Cfg, out: &[CopyFacts], reachable: &[bool]) -> CopyFacts {
    /* Function entry starts without any copy information.  */
    if block == 0 {
        return HashMap::new();
    }

    /* Only consider reachable predecessors.  */
    let mut preds = cfg.blocks[block]
        .preds
        .iter()
        .map(|id| id.0)
        .filter(|&id| reachable[id]);

    let Some(first) = preds.next() else {
        return HashMap::new();
    };

    /* Start with the first predecessor's facts.  */
    let mut facts = out[first].clone();

    /* Intersect with every other reachable predecessor.  */
    for pred in preds {
        facts.retain(|name, source| out[pred].get(name) == Some(source));
    }

    facts
}

fn transfer_copy(instr: &IRInstr, facts: &mut CopyFacts) {
    let Some(IRVal::Var {
        name: dst,
        ty: dst_ty,
    }) = instr.dst()
    else {
        return;
    };

    /* Determine whether this instruction introduces a new copy.
     * Only propagate copies where no type conversion occurs.  */
    let new_copy = match instr {
        IRInstr::Cpy {
            src: IRVal::Var {
                name: src,
                ty: src_ty,
            },
            src_ty: declared_ty,
            ..
        } if src_ty == declared_ty && dst_ty == declared_ty && src != dst => {
            Some((src.clone(), *src_ty))
        }

        _ => None,
    };

    /* Invalidate every relationship involving the overwritten
     * destination, either as a destination or as a source.  */
    facts.retain(|name, (source, _)| name != dst && source != dst);

    /* Record the new relationship, if any.  */
    if let Some(source) = new_copy {
        facts.insert(dst.clone(), source);
    }
}
