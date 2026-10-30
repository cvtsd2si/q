use std::collections::HashMap;

use crate::{
    ir::{
        cfg::Cfg,
        r#gen::{IRFunc, IRInstr, IRVal},
    },
    parser::ast::{Literal, Type},
    runtime::value::Value,
};

use super::visit_reads::visit_reads;

pub(super) fn run(func: &mut IRFunc) -> bool {
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
                if let IRVal::Var { name, ty } = val
                    && let Some(&konst) = facts.get(name)
                {
                    /* Never replace a typed read with a different type.  */
                    if Into::<Type>::into(konst) == *ty {
                        *val = IRVal::Const(konst);
                        changed = true;
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
