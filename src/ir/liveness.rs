use std::collections::HashSet;

use crate::ir::{
    cfg::Cfg,
    r#gen::{IRFunc, IRInstr, IRVal},
};

#[derive(Debug, Default)]
pub struct BlockLiveness {
    pub live_in: HashSet<String>,
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

        for (id, ud) in block_use_defs.iter().enumerate() {
            println!("Block {id}:");
            println!("\tUSES = {:?}", ud.uses);
            println!("\tDEFS = {:?}", ud.defs);
        }

        Self {
            blocks: (0..cfg.blocks.len())
                .map(|_| BlockLiveness::default())
                .collect(),
        }
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
