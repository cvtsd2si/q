mod constant_folding;
mod constant_propagation;
mod copy_propagation;
mod dataflow;
mod dead_store_elimination;
mod unreachable_code_elimination;
mod visit_reads;

use std::str::FromStr;

use crate::ir::r#gen::IRProg;

#[derive(Debug, Clone, Copy)]
pub enum Optimization {
    ConstantFolding,
    ConstantPropagation,
    CopyPropagation,
    DeadStoreElimination,
    UnreachableCodeElimination,
}

impl FromStr for Optimization {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let opt = match s {
            "constant_folding" => Optimization::ConstantFolding,
            "constant_propagation" => Optimization::ConstantPropagation,
            "copy_propagation" => Optimization::CopyPropagation,
            "dead_store_elimination" => Optimization::DeadStoreElimination,
            "unreachable_code_elimination" => Optimization::UnreachableCodeElimination,
            _ => return Err(format!("cannot recognize {s} as optimization")),
        };

        Ok(opt)
    }
}

pub fn optimize_ir(mut ir: IRProg, enabled: &[Optimization]) -> IRProg {
    for func in &mut ir.funcs {
        loop {
            let mut changed = false;

            for pass in enabled {
                changed |= match pass {
                    Optimization::ConstantFolding => constant_folding::run(func),
                    Optimization::ConstantPropagation => constant_propagation::run(func),
                    Optimization::CopyPropagation => copy_propagation::run(func),
                    Optimization::UnreachableCodeElimination => {
                        unreachable_code_elimination::run(func)
                    }
                    Optimization::DeadStoreElimination => dead_store_elimination::run(func),
                };
            }

            if !changed {
                break;
            }
        }
    }

    ir
}
