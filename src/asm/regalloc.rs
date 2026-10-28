use crate::{
    asm::r#gen::{AsmFunc, AsmInstr, AsmOp, AsmProg, AsmReg},
    parser::ast::Type,
};
use std::collections::HashMap;

pub struct RegAllocator {
    allocatable_gp_regs: Vec<AsmReg>,
    allocatable_sse_regs: Vec<AsmReg>,
    map: HashMap<String, AsmOp>,
    stack_offsets: HashMap<String, isize>,
    current_offset: isize,
}

impl RegAllocator {
    pub fn new() -> Self {
        Self {
            allocatable_gp_regs: vec![AsmReg::R8, AsmReg::R11], /* %r9, %r10 reserved for fixup scratch  */
            allocatable_sse_regs: vec![
                AsmReg::Xmm0,
                AsmReg::Xmm1,
                AsmReg::Xmm2,
                AsmReg::Xmm3,
                AsmReg::Xmm4,
                AsmReg::Xmm5,
                AsmReg::Xmm6,
                AsmReg::Xmm7,
            ],
            map: HashMap::new(),
            stack_offsets: HashMap::new(),
            current_offset: 0,
        }
    }

    pub fn rewrite(&mut self, instr: &mut AsmInstr) -> AsmInstr {
        match instr {
            AsmInstr::Mov { src, dst, asm_ty } => AsmInstr::Mov {
                src: self.replace_pseudo(src),
                dst: self.replace_pseudo(dst),
                asm_ty: *asm_ty,
            },

            AsmInstr::Movsx {
                src,
                src_ty,
                dst,
                dst_ty,
            } => AsmInstr::Movsx {
                src: self.replace_pseudo(src),
                src_ty: *src_ty,
                dst: self.replace_pseudo(dst),
                dst_ty: *dst_ty,
            },

            AsmInstr::Movzx {
                src,
                src_ty,
                dst,
                dst_ty,
            } => AsmInstr::Movzx {
                src: self.replace_pseudo(src),
                src_ty: *src_ty,
                dst: self.replace_pseudo(dst),
                dst_ty: *dst_ty,
            },

            AsmInstr::Unary { kind, dst, asm_ty } => AsmInstr::Unary {
                kind: *kind,
                dst: self.replace_pseudo(dst),
                asm_ty: *asm_ty,
            },

            AsmInstr::Div { op, asm_ty } => AsmInstr::Div {
                op: self.replace_pseudo(op),
                asm_ty: *asm_ty,
            },

            AsmInstr::Idiv { op, asm_ty } => AsmInstr::Idiv {
                op: self.replace_pseudo(op),
                asm_ty: *asm_ty,
            },

            AsmInstr::Bin {
                kind,
                lhs,
                rhs,
                asm_ty,
            } => AsmInstr::Bin {
                kind: kind.to_owned(),
                lhs: self.replace_pseudo(lhs),
                rhs: self.replace_pseudo(rhs),
                asm_ty: *asm_ty,
            },

            AsmInstr::Cmp { src, dst, asm_ty } => AsmInstr::Cmp {
                src: self.replace_pseudo(src),
                dst: self.replace_pseudo(dst),
                asm_ty: *asm_ty,
            },

            AsmInstr::Setcc { cc, dst } => AsmInstr::Setcc {
                cc: *cc,
                dst: self.replace_pseudo(dst),
            },

            AsmInstr::Shift {
                kind,
                src,
                dst,
                asm_ty,
            } => AsmInstr::Shift {
                kind: *kind,
                src: src.clone(),
                dst: self.replace_pseudo(dst),
                asm_ty: *asm_ty,
            },

            AsmInstr::Cvtsi2sd { src, dst, asm_ty } => AsmInstr::Cvtsi2sd {
                src: self.replace_pseudo(src),
                dst: self.replace_pseudo(dst),
                asm_ty: *asm_ty,
            },

            AsmInstr::Cvtsi2ss { src, dst, asm_ty } => AsmInstr::Cvtsi2ss {
                src: self.replace_pseudo(src),
                dst: self.replace_pseudo(dst),
                asm_ty: *asm_ty,
            },

            AsmInstr::Cvttsd2si { src, dst, asm_ty } => AsmInstr::Cvttsd2si {
                src: self.replace_pseudo(src),
                dst: self.replace_pseudo(dst),
                asm_ty: *asm_ty,
            },

            AsmInstr::Cvttss2si { src, dst, asm_ty } => AsmInstr::Cvttss2si {
                src: self.replace_pseudo(src),
                dst: self.replace_pseudo(dst),
                asm_ty: *asm_ty,
            },

            AsmInstr::Cvtss2sd { src, dst } => AsmInstr::Cvtss2sd {
                src: self.replace_pseudo(src),
                dst: self.replace_pseudo(dst),
            },

            AsmInstr::Cvtsd2ss { src, dst } => AsmInstr::Cvtsd2ss {
                src: self.replace_pseudo(src),
                dst: self.replace_pseudo(dst),
            },

            _ => instr.clone(),
        }
    }

    fn replace_pseudo(&mut self, op: &mut AsmOp) -> AsmOp {
        match op {
            AsmOp::Pseudo { name, ty } => self.alloc(name, ty),
            _ => op.clone(),
        }
    }

    fn alloc(&mut self, pseudo: &str, ty: &Type) -> AsmOp {
        if let Some(op) = self.map.get(pseudo) {
            return op.clone();
        }

        let reg = match ty {
            Type::Float32 | Type::Float64 => self.allocatable_sse_regs.pop().map(AsmOp::Reg),
            _ => self.allocatable_gp_regs.pop().map(AsmOp::Reg),
        };

        let op = reg.unwrap_or_else(|| AsmOp::Mem(AsmReg::Bp, self.get_stack_offset(pseudo)));

        self.map.insert(pseudo.to_owned(), op.clone());

        op
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
    fn regalloc(self) -> Self::Output;
}

impl Regalloc for AsmProg {
    type Output = AsmProg;

    fn regalloc(self) -> Self::Output {
        AsmProg {
            funcs: self.funcs.into_iter().map(|f| f.regalloc()).collect(),
            static_consts: self.static_consts.clone(),
        }
    }
}

impl Regalloc for AsmFunc {
    type Output = AsmFunc;

    fn regalloc(self) -> Self::Output {
        let mut allocator = RegAllocator::new();

        let mut f = AsmFunc {
            name: self.name.clone(),
            body: self
                .body
                .into_iter()
                .map(|ref mut instr| allocator.rewrite(instr))
                .collect(),
            space: 0,
        };

        f.space = allocator.get_space2reserve();

        f
    }
}
