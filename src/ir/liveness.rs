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
        for (idx, instr) in func.body.iter().enumerate() {
            let ud = instr.use_def();

            println!("{idx}: {instr:?}");
            println!("\tuses: {:?}", ud.uses);
            println!("\tdefs: {:?}", ud.defs);
        }

        Self { blocks: Vec::new() }
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
