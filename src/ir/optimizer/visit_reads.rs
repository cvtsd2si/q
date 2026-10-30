use crate::ir::r#gen::{IRInstr, IRVal};

pub(super) fn visit_reads(instr: &mut IRInstr, mut visit: impl FnMut(&mut IRVal)) {
    match instr {
        IRInstr::Cpy { src, .. }
        | IRInstr::Unary { src, .. }
        | IRInstr::SignExtend { src, .. }
        | IRInstr::ZeroExtend { src, .. }
        | IRInstr::Truncate { src, .. }
        | IRInstr::DoubleToInt { src, .. }
        | IRInstr::IntToDouble { src, .. }
        | IRInstr::DoubleToUInt { src, .. }
        | IRInstr::UIntToDouble { src, .. }
        | IRInstr::DoubleToFloat { src, .. }
        | IRInstr::FloatToDouble { src, .. }
        | IRInstr::FloatToInt { src, .. }
        | IRInstr::IntToFloat { src, .. }
        | IRInstr::FloatToUInt { src, .. }
        | IRInstr::UIntToFloat { src, .. } => visit(src),

        IRInstr::Bin { lhs, rhs, .. } => {
            visit(lhs);
            visit(rhs);
        }
        IRInstr::Ret { val: Some(val), .. }
        | IRInstr::Print { val, .. }
        | IRInstr::Jz { cond: val, .. } => visit(val),
        IRInstr::Call { args, .. } => {
            for arg in args {
                visit(arg);
            }
        }
        IRInstr::Ret { val: None, .. } | IRInstr::Label(_) | IRInstr::Jmp { .. } => {}
    }
}
