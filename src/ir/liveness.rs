use std::collections::HashSet;

use crate::ir::{
    cfg::Cfg,
    r#gen::{IRFunc, IRInstr, IRVal},
};

#[derive(Debug, Default)]
pub struct BlockLiveness {
    /* Variables whose current values may be needed
     * when entering this block.  */
    pub live_in: HashSet<String>,
    /* Variables whose current values may be needed
     * after leaving this block.  */
    pub live_out: HashSet<String>,
}

#[derive(Debug)]
pub struct Liveness {
    pub blocks: Vec<BlockLiveness>,
}

impl Liveness {
    pub fn analyze(func: &IRFunc, cfg: &Cfg) -> Self {
        let mut block_use_defs = Vec::new();

        for block in &cfg.blocks {
            let mut block_ud = UseDef::default();

            for idx in block.range.clone() {
                let instr_ud = func.body[idx].use_def();

                /* Does this instruction use a variable that hasn't already
                 * been defined earlier in this block?
                 *
                 * If it does, the variable must be available when execution
                 * enters this block.  */
                for var in instr_ud.uses {
                    if !block_ud.defs.contains(&var) {
                        block_ud.uses.insert(var);
                    }
                }

                /* Record the definitions we've encountered so far.  */
                block_ud.defs.extend(instr_ud.defs);
            }

            block_use_defs.push(block_ud);
        }

        let mut blocks: Vec<BlockLiveness> = (0..cfg.blocks.len())
            .map(|_| BlockLiveness::default())
            .collect();

        /* Fixed-point iteration.  */
        let mut iterations = 0;
        loop {
            iterations += 1;

            let mut changed = false;

            /* Visit the loop backwards, since liveness is a backward dataflow
             * analysis.  The information flows from successors to predecessors.  */
            for id in (0..cfg.blocks.len()).rev() {
                /* OUT[B] = union of IN[S] for every successor S.  */
                let mut live_out = HashSet::new();

                for successor in &cfg.blocks[id].succs {
                    live_out.extend(blocks[successor.0].live_in.iter().cloned());
                }

                /* IN[B] = USE[B] U (OUT[B] - DEF[B])  */

                /* First, include everything used before being defined.  */
                let mut live_in = block_use_defs[id].uses.clone();

                /* Then, add variables that are live-out but not defined inside this block.  */
                for var in &live_out {
                    if !block_use_defs[id].defs.contains(var) {
                        live_in.insert(var.clone());
                    }
                }

                /* If either set differs from its previous value, update it.  */
                if blocks[id].live_in != live_in || blocks[id].live_out != live_out {
                    blocks[id].live_in = live_in;
                    blocks[id].live_out = live_out;

                    changed = true;
                }
            }

            if !changed {
                break;
            }
        }

        println!("Liveness converged after {iterations} attempts.");

        Self { blocks }
    }
}

#[derive(Debug, Default)]
pub struct UseDef {
    pub uses: HashSet<String>,
    pub defs: HashSet<String>,
}

impl UseDef {
    fn add_use(&mut self, val: &IRVal) {
        if let IRVal::Var { name, .. } = val {
            self.uses.insert(name.clone());
        }
    }

    fn add_def(&mut self, val: &IRVal) {
        if let IRVal::Var { name, .. } = val {
            self.defs.insert(name.clone());
        }
    }
}

impl IRInstr {
    pub fn use_def(&self) -> UseDef {
        let mut ud = UseDef::default();

        match self {
            Self::Cpy { src, dst, .. }
            | Self::Unary { src, dst, .. }
            | Self::SignExtend { src, dst, .. }
            | Self::ZeroExtend { src, dst, .. }
            | Self::Truncate { src, dst, .. }
            | Self::DoubleToInt { src, dst, .. }
            | Self::IntToDouble { src, dst, .. }
            | Self::DoubleToUInt { src, dst, .. }
            | Self::UIntToDouble { src, dst, .. }
            | Self::DoubleToFloat { src, dst, .. }
            | Self::FloatToDouble { src, dst, .. }
            | Self::FloatToInt { src, dst, .. }
            | Self::IntToFloat { src, dst, .. }
            | Self::FloatToUInt { src, dst, .. }
            | Self::UIntToFloat { src, dst, .. } => {
                ud.add_use(src);
                ud.add_def(dst);
            }

            Self::Bin { lhs, rhs, dst, .. } => {
                ud.add_use(lhs);
                ud.add_use(rhs);
                ud.add_def(dst);
            }

            Self::Ret { val, .. } => {
                if let Some(val) = val {
                    ud.add_use(val);
                }
            }

            Self::Print { val, .. } => {
                ud.add_use(val);
            }

            Self::Jz { cond, .. } => {
                ud.add_use(cond);
            }

            Self::Call { args, dst, .. } => {
                for arg in args {
                    ud.add_use(arg);
                }

                ud.add_def(dst);
            }

            Self::Jmp { .. } | Self::Label(_) => {}
        }

        ud
    }
}
