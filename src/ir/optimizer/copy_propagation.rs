use std::collections::HashMap;

use crate::{
    ir::r#gen::{IRFunc, IRInstr, IRVal},
    parser::ast::Type,
};

use super::{
    dataflow::run_forward,
    visit_reads::visit_reads,
};

type CopyFacts = HashMap<String, (String, Type)>;

pub(super) fn run(func: &mut IRFunc) -> bool {
    run_forward::<CopyFacts>(
        func,
        intersect_copies,
        |a, b| a == b,
        transfer_copy,
        rewrite_copies,
    )
}

fn intersect_copies(
    facts: &mut CopyFacts,
    other: &CopyFacts,
) {
    facts.retain(|name, source| {
        other.get(name) == Some(source)
    });
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

fn rewrite_copies(
    instr: &mut IRInstr,
    facts: &CopyFacts,
) -> bool {
    let mut changed = false;

    visit_reads(instr, |val| {
        if let IRVal::Var { name, ty } = val
            && let Some((source, source_ty)) =
                facts.get(name.as_str())
            && *ty == *source_ty
            && name.as_str() != source.as_str()
        {
            *val = IRVal::Var {
                name: source.clone(),
                ty: *source_ty,
            };

            changed = true;
        }
    });

    changed
}
