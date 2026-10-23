use crate::asm::r#gen::{AsmFunc, AsmInstr, AsmOp, AsmProg, AsmReg};
use std::collections::HashMap;

pub struct RegAllocator {
    allocatable_regs: Vec<AllocatableReg>,
    map: HashMap<String, AsmReg>,
    stack_offsets: HashMap<String, isize>,
    current_offset: isize,
}

#[derive(Debug, Clone, Copy)]
struct AllocatableReg {
    reg: AsmReg,
    occupied: bool,
}

impl RegAllocator {
    pub fn new() -> Self {
        Self {
            allocatable_regs:
                [AsmReg::R8, AsmReg::R10, AsmReg::R11] /* R9 reserved for fixup scratch */
                    .iter()
                    .map(|r| AllocatableReg {
                        reg: r.to_owned(),
                        occupied: false,
                    })
                    .collect(),
            map: HashMap::new(),
            stack_offsets: HashMap::new(),
            current_offset: 0,
        }
    }

    pub fn rewrite(&mut self, instr: &mut AsmInstr) -> AsmInstr {
        match instr {
            AsmInstr::Mov { src, dst } => AsmInstr::Mov {
                src: self.replace_pseudo(src),
                dst: self.replace_pseudo(dst),
            },

            AsmInstr::Bin { kind, lhs, rhs } => AsmInstr::Bin {
                kind: kind.to_owned(),
                lhs: self.replace_pseudo(lhs),
                rhs: self.replace_pseudo(rhs),
            },

            AsmInstr::Cmp { src, dst } => AsmInstr::Cmp {
                src: self.replace_pseudo(src),
                dst: self.replace_pseudo(dst),
            },

            _ => instr.clone(),
        }
    }

    fn replace_pseudo(&mut self, op: &mut AsmOp) -> AsmOp {
        match op {
            AsmOp::Pseudo(pseudo) => self.alloc(pseudo),
            _ => op.clone(),
        }
    }

    fn alloc(&mut self, pseudo: &str) -> AsmOp {
        if let Some(reg) = self.map.get(pseudo) {
            return AsmOp::Reg(reg.to_owned());
        }

        let slot = self.allocatable_regs.iter_mut().find(|reg| !reg.occupied);

        if slot.is_none() {
            return AsmOp::Stack(self.get_stack_offset(pseudo));
        }

        let slot = slot.unwrap();

        slot.occupied = true;

        self.map.insert(pseudo.to_owned(), slot.reg.to_owned());

        AsmOp::Reg(slot.reg)
    }

    fn get_stack_offset(&mut self, pseudo: &str) -> isize {
        if let Some(offset) = self.stack_offsets.get(pseudo) {
            return *offset;
        }

        let next_offset = self.get_next_stack_offset();

        self.stack_offsets.insert(pseudo.to_owned(), next_offset);

        next_offset
    }

    fn get_next_stack_offset(&mut self) -> isize {
        self.current_offset -= 8;
        self.current_offset
    }

    pub fn get_space2reserve(&self) -> usize {
        let size = self.current_offset.unsigned_abs();

        size.div_ceil(16) * 16
    }
}

impl Default for RegAllocator {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Regalloc {
    type Output;

    fn regalloc(&mut self) -> Self::Output;
}

impl Regalloc for AsmProg {
    type Output = AsmProg;

    fn regalloc(&mut self) -> Self::Output {
        AsmProg {
            funcs: self.funcs.iter_mut().map(|f| f.regalloc()).collect(),
        }
    }
}

impl Regalloc for AsmFunc {
    type Output = AsmFunc;

    fn regalloc(&mut self) -> Self::Output {
        let mut allocator = RegAllocator::new();

        let mut f = AsmFunc {
            name: self.name.clone(),
            body: self
                .body
                .iter_mut()
                .map(|instr| allocator.rewrite(instr))
                .collect(),
            space: 0,
        };

        f.space = allocator.get_space2reserve();

        f
    }
}
