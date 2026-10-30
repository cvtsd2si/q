use crate::analysis::cfg::Cfg;
use crate::ir::r#gen::{IRFunc, IRInstr};

pub(super) fn run_forward<F: Clone + Default>(
    func: &mut IRFunc,
    intersect: fn(&mut F, &F),
    same: fn(&F, &F) -> bool,
    transfer: fn(&IRInstr, &mut F),
    rewrite: fn(&mut IRInstr, &F) -> bool,
) -> bool {
    let cfg = Cfg::build(&func.body).unwrap();
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

        stack.extend(cfg.blocks[block_idx].succs.iter().map(|id| id.0));
    }

    let mut out = vec![F::default(); n];

    loop {
        let mut analysis_changed = false;

        for (idx, block) in cfg.blocks.iter().enumerate() {
            if !reachable[idx] {
                continue;
            }

            let mut facts = incoming(idx, &cfg, &out, &reachable, intersect);

            for i in block.range.clone() {
                transfer(&func.body[i], &mut facts);
            }

            if !same(&out[idx], &facts) {
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

        let mut facts = incoming(idx, &cfg, &out, &reachable, intersect);

        for i in block.range.clone() {
            let instr = &mut func.body[i];

            changed |= rewrite(instr, &facts);

            transfer(instr, &mut facts);
        }
    }

    changed
}

fn incoming<F: Clone + Default>(
    block: usize,
    cfg: &Cfg,
    out: &[F],
    reachable: &[bool],
    intersect: fn(&mut F, &F),
) -> F {
    if block == 0 {
        return F::default();
    }

    let mut preds = cfg.blocks[block]
        .preds
        .iter()
        .map(|id| id.0)
        .filter(|&id| reachable[id]);

    let Some(first) = preds.next() else {
        return F::default();
    };

    let mut facts = out[first].clone();

    for pred in preds {
        intersect(&mut facts, &out[pred]);
    }

    facts
}
