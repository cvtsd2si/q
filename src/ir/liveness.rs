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

#[derive(Debug, Default)]
pub struct InstrLiveness {
    /* Variables whose current values may be needed
     * when executing this instruction.  */
    pub live_in: HashSet<String>,
    /* Variables whose current values may be needed
     * after executing this instruction.  */
    pub live_out: HashSet<String>,
}

#[derive(Debug)]
pub struct Liveness {
    pub blocks: Vec<BlockLiveness>,
    pub instrs: Vec<InstrLiveness>,
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

        /* Calculate instruction-level liveness.  */
        let mut instrs: Vec<InstrLiveness> = (0..func.body.len())
            .map(|_| InstrLiveness::default())
            .collect();

        /* Consider:
         *
         * ```
         * 0: a = 10
         * 1: b = 20
         * 2: c = a + b
         * 3: print c
         * ```
         *
         * We're working backward from instr #3.
         * Nothing happens afterward, so LIVE_OUT[3] = {}.
         * However, `print c` needs `c`, therefore LIVE_IN[3] = {c}.
         *
         * Now, move backward to instr #2.
         *
         *  live = {c}
         *
         * First, we save LIVE_OUT:
         *
         *  instrs[2].live_out = live.clone()
         *
         * Next, we remove defs:
         *
         *  live.retain(|var| !ud.defs.contains(var);
         *
         * Since instr #2 defines `c`, we remove it:
         *
         *  live = {}
         *
         * Since instr #2 uses `a` and `b`, we add them:
         *
         *  live = {a, b}
         *
         * We save this as LIVE_IN for instr #2:
         *
         *  instrs[2].live_in = live.clone()
         *
         * Now, move backward to instr #1.
         *
         *  live = {a, b}
         *
         * First, we save LIVE_OUT:
         *
         *  instrs[1].live_out = live.clone()
         *
         * Next, we remove defs:
         *
         *  live.retain(|var| !ud.defs.contains(var);
         *
         * Since instr #1 defines `b`, we remove it:
         *
         *  live = {a}
         *
         * Instr #1 does not use anything, so we do not add anything.
         *
         *  live = {a}
         *
         * We save this as LIVE_IN for instr #1:
         *
         *  instrs[1].live_in = live.clone()
         *
         * Now, we move backward to instr #0:
         *
         *  live = {a}
         *
         * First, we save LIVE_OUT:
         *
         *  instrs[0].live_out = live.clone()
         *
         * Next, we remove defs:
         *
         *  live.retain(|var| !ud.defs.contains(var);
         *
         * Since instr #0 defines `a`, we remove it:
         *
         *  live = {}
         *
         * Instr #0 does not use anything, so we do not add anything.
         *
         *  live = {}
         *
         * There are no more instructions to move backward to, so we're done here.
         *
         * Complete result:
         *
         * ```                  LIVE_IN         LIVE_OUT
         * 0: a = 10            {}              {a}
         * 1: b = 20            {a}             {a, b}
         * 2: c = a + b         {a, b}          {c}
         * 3: print c           {c}             {}
         * ```
         */
        for (id, block) in cfg.blocks.iter().enumerate() {
            /* Start with this block's LIVE_OUT set, which currently correctly
             * represents the variables needed after the block ends.  */
            let mut live = blocks[id].live_out.clone();

            /* Walk through the instructions backward.  */
            for idx in block.range.clone().rev() {
                /* Get current instruction's uses and defs.  */
                let ud = func.body[idx].use_def();

                /* Record LIVE_OUT for this instruction.  */
                instrs[idx].live_out = live.clone();

                /* Remove variables defined by this instruction.  */
                live.retain(|var| !ud.defs.contains(var));

                /* Add variables used by this instruction.  */
                live.extend(ud.uses);

                /* Record LIVE_IN for this instruction.  */
                instrs[idx].live_in = live.clone();
            }

            /* This invariant must hold.  */
            assert_eq!(live, blocks[id].live_in);
        }

        Self { blocks, instrs }
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
