use std::collections::HashMap;

use crate::{
    ir::r#gen::{IRFunc, IRInstr, IRVal},
    parser::ast::{Literal, Type},
    runtime::value::Value,
};

use super::{
    dataflow::run_forward,
    visit_reads::visit_reads,
};

type ConstantFacts = HashMap<String, Literal>;

pub(super) fn run(func: &mut IRFunc) -> bool {
    run_forward::<ConstantFacts>(
        func,
        intersect_constants,
        same_constants,
        transfer_constant,
        rewrite_constants,
    )
}

fn intersect_constants(
    facts: &mut ConstantFacts,
    other: &ConstantFacts,
) {
    facts.retain(|name, value| {
        other
            .get(name)
            .is_some_and(|other_value| {
                same_constant(*value, *other_value)
            })
    });
}

fn rewrite_constants(
    instr: &mut IRInstr,
    facts: &ConstantFacts,
) -> bool {
    let mut changed = false;

    visit_reads(instr, |val| {
        if let IRVal::Var { name, ty } = val
            && let Some(&constant) = facts.get(name)
            && Into::<Type>::into(constant) == *ty
        {
            *val = IRVal::Const(constant);
            changed = true;
        }
    });

    changed
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
