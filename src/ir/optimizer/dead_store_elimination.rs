use crate::{
    analysis::cfg::Cfg,
    analysis::liveness::Liveness,
    ir::r#gen::{IRFunc, IRInstr, IRVal},
    parser::ast::BinaryExpressionKind,
};

pub(super) fn run(func: &mut IRFunc) -> bool {
    let cfg = Cfg::build(&func.body).unwrap();
    let liveness = Liveness::<String>::analyze(&func.body, &cfg);

    let original_len = func.body.len();
    let mut idx = 0;

    func.body.retain(|instr| {
        let live_out = &liveness.instrs[idx].live_out;
        idx += 1;

        /* Check whether the instruction defines a variable
         * that is not live afterward.  */
        let dead = match instr.dst() {
            Some(IRVal::Var { name, .. }) => !live_out.contains(name),

            _ => false,
        };

        /* Remove only dead assignments without side effects.  */
        !(dead && is_pure(instr))
    });

    func.body.len() != original_len
}

fn is_pure(instr: &IRInstr) -> bool {
    match instr {
        IRInstr::Cpy { .. }
        | IRInstr::Unary { .. }
        | IRInstr::SignExtend { .. }
        | IRInstr::ZeroExtend { .. }
        | IRInstr::Truncate { .. }
        | IRInstr::DoubleToInt { .. }
        | IRInstr::IntToDouble { .. }
        | IRInstr::DoubleToUInt { .. }
        | IRInstr::UIntToDouble { .. }
        | IRInstr::DoubleToFloat { .. }
        | IRInstr::FloatToDouble { .. }
        | IRInstr::FloatToInt { .. }
        | IRInstr::IntToFloat { .. }
        | IRInstr::FloatToUInt { .. }
        | IRInstr::UIntToFloat { .. } => true,

        IRInstr::Bin { kind, .. } => {
            !matches!(kind, BinaryExpressionKind::Div | BinaryExpressionKind::Rem)
        }

        _ => false,
    }
}
