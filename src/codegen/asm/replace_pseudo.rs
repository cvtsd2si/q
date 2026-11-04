use std::collections::HashMap;

use crate::{
    codegen::asm::r#gen::{AsmFunc, AsmOp, AsmReg},
    parser::ast::TypeInterner,
};

pub trait ReplacePseudo {
    fn replace_pseudo(self, types: &TypeInterner) -> Self;
}

impl ReplacePseudo for AsmFunc {
    fn replace_pseudo(mut self, types: &TypeInterner) -> Self {
        let mut slots = HashMap::<String, isize>::new();
        let mut used = self.space;

        for instr in &mut self.body {
            for operand in instr.operands_mut().into_iter().flatten() {
                let AsmOp::PseudoMem(name, offset, ty) = operand else {
                    continue;
                };

                let displacement = *offset;

                let base = *slots.entry(name.clone()).or_insert_with(|| {
                    let size = ty.size_of(types);
                    let alignment = ty.alignment(types);

                    used = (used + size).div_ceil(alignment) * alignment;

                    -(used as isize)
                });

                *operand = AsmOp::Mem(AsmReg::Bp, base + displacement);
            }
        }

        self.space = used;
        self
    }
}
