use std::collections::HashMap;

use crate::{
    ir::{
        cfg::Cfg,
        r#gen::{IRFunc, IRInstr, IRVal},
    },
    parser::ast::Type,
};

use super::visit_reads::visit_reads;

type CopyFacts = HashMap<String, (String, Type)>;

pub(super) fn run(func: &mut IRFunc) -> bool {
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
