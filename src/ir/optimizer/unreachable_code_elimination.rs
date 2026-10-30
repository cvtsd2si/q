use crate::analysis::cfg::Cfg;
use crate::ir::r#gen::IRFunc;

pub(super) fn run(func: &mut IRFunc) -> bool {
    let cfg = Cfg::build(&func.body).unwrap();
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
