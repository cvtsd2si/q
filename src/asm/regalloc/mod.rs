mod coalescing;
mod interference;

use self::coalescing::{Coalescing, collect_moves, collect_precolored_moves};
use self::interference::InterferenceNode;
use crate::analysis::cfg::Cfg;
use crate::analysis::liveness::{Liveness, LivenessInstruction};
use crate::asm::r#gen::{AsmInstr, AsmInstrUnaryKind, AsmReg, AsmType, CallConv, RtFn};
use crate::asm::regalloc::interference::InterferenceGraph;
use crate::parser::ast::Type;
use crate::{analysis::liveness::UseDef, asm::r#gen::AsmOp};
use crate::{asm::r#gen::AsmFunc, ir::r#gen::IRFunc};
use std::collections::{BTreeMap, BTreeSet};

pub trait Regalloc {
    fn regalloc(self, ir: &IRFunc) -> AsmFunc;
}

impl Regalloc for AsmFunc {
    fn regalloc(mut self, _ir: &IRFunc) -> AsmFunc {
        // 1. Build the control-flow graph.
        let cfg = Cfg::build(&self.body).expect("Failed to build assembly CFG");

        // 2. Compute liveness.
        let liveness = Liveness::<AsmLocation>::analyze(&self.body, &cfg);

        // 3. Build the interference graph.
        let graph = InterferenceGraph::build(&self.body, &liveness);

        // 4. Determine register classes.
        let classes = collect_reg_classes(&self.body).expect("Failed to collect register classes");

        // 5. Coalesce compatible pseudo-registers.
        let mut coalescing = Coalescing::new(graph);

        // Collect move candidates once.
        // The instructions themselves do not change during coalescing.
        let moves = collect_moves(&self.body);
        let precolored_moves = collect_precolored_moves(&self.body);

        loop {
            // Remember the number of nodes before this pass.
            let before = coalescing.graph.neighbors.len();

            // Attempt pseudo-to-pseudo coalescing.
            //
            // Depending on current representatives, try_coalesce()
            // may also delegate to George's criterion.
            for mov in &moves {
                coalescing.try_coalesce(mov, &classes);
            }

            // Attempt pseudo-to-precolored coalescing.
            for (name, reg) in &precolored_moves {
                coalescing.try_coalesce_precolored(name, *reg, &classes);
            }

            // A successful merge removes exactly one graph node.
            //
            // If the node count has not changed, this pass made
            // no progress and we have reached a fixed point.
            if coalescing.graph.neighbors.len() == before {
                break;
            }
        }

        // 6. Collect spill costs for surviving graph nodes.
        let original_costs = collect_spill_costs(&self.body);

        let mut spill_costs = BTreeMap::new();

        for (name, cost) in original_costs {
            let representative = coalescing
                .representative
                .get(&name)
                .expect("Missing coalescing representative");

            if let InterferenceNode::Pseudo(rep) = representative {
                *spill_costs.entry(rep.clone()).or_insert(0) += cost;
            }
        }

        // 7. Color the coalesced graph.
        let (colored, spilled) = coalescing.graph.color(&classes, &spill_costs);

        // 8. Assign stack slots to spilled representatives.
        let spill_sizes = collect_spill_sizes(&self.body, &coalescing.representative);
        let (spill_slots, space) = assign_spill_slots(&spilled, &spill_sizes, self.space);

        // 9. Restore locations for every original pseudo.
        let mut assignments = BTreeMap::new();
        let mut slots = BTreeMap::new();

        for (name, representative) in &coalescing.representative {
            match representative {
                // This pseudo was merged into a physical register.
                InterferenceNode::Reg(reg) => {
                    assignments.insert(name.clone(), *reg);
                }

                // This pseudo belongs to a surviving pseudo node.
                InterferenceNode::Pseudo(rep) => {
                    if let Some(&reg) = colored.get(rep) {
                        assignments.insert(name.clone(), reg);
                    } else if let Some(&offset) = spill_slots.get(rep) {
                        slots.insert(name.clone(), offset);
                    } else {
                        panic!("No allocation for {name}, representative {rep}");
                    }
                }
            }
        }

        // 10. Rewrite assembly operands.
        for instr in &mut self.body {
            instr.rewrite_colored(&assignments);
            instr.rewrite_spilled(&slots);
        }

        // 11. Remove copies between identical physical locations.
        self.body.retain(|instr| !is_redundant_move(instr));

        // 12. Update the required stack-frame size.
        self.space = space;

        for instr in &self.body {
            for operand in instr.operands().into_iter().flatten() {
                assert!(
                    !matches!(operand, AsmOp::Pseudo { .. }),
                    "Unallocated pseudo-register in {instr:?}"
                );
            }
        }

        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegClass {
    Gp,
    Sse,
}

impl RegClass {
    fn of_type(ty: &Type) -> Self {
        match ty {
            Type::Float32 | Type::Float64 => Self::Sse,
            _ => Self::Gp,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AsmLocation {
    Pseudo(String),
    Reg(AsmReg),
    Flags,
}

impl AsmOp {
    fn add_uses(&self, ud: &mut UseDef<AsmLocation>) {
        match self {
            Self::Pseudo { name, .. } => {
                ud.uses.insert(AsmLocation::Pseudo(name.clone()));
            }
            Self::Reg(reg) | Self::Mem(reg, _) => {
                ud.uses.insert(AsmLocation::Reg(*reg));
            }
            Self::Imm(_) | Self::Data(_, _) => {}
        }
    }

    fn add_defs(&self, ud: &mut UseDef<AsmLocation>) {
        match self {
            Self::Pseudo { name, .. } => {
                ud.defs.insert(AsmLocation::Pseudo(name.clone()));
            }
            Self::Reg(reg) => {
                ud.defs.insert(AsmLocation::Reg(*reg));
            }
            Self::Mem(reg, _) => {
                ud.uses.insert(AsmLocation::Reg(*reg));
            }
            Self::Imm(_) | Self::Data(_, _) => {}
        }
    }
}

impl LivenessInstruction for AsmInstr {
    type Location = AsmLocation;

    fn use_def(&self) -> UseDef<Self::Location> {
        let mut ud = UseDef::<AsmLocation>::default();

        match self {
            AsmInstr::Mov { src, dst, .. } => {
                src.add_uses(&mut ud);
                dst.add_defs(&mut ud);
            }

            AsmInstr::Bin {
                lhs, rhs, asm_ty, ..
            } => {
                lhs.add_uses(&mut ud);
                rhs.add_uses(&mut ud);
                rhs.add_defs(&mut ud);

                if !matches!(asm_ty, AsmType::Float | AsmType::Double) {
                    ud.defs.insert(AsmLocation::Flags);
                }
            }

            AsmInstr::Unary { kind, dst, .. } => {
                dst.add_uses(&mut ud);
                dst.add_defs(&mut ud);

                match kind {
                    AsmInstrUnaryKind::Neg | AsmInstrUnaryKind::Shr => {
                        ud.defs.insert(AsmLocation::Flags);
                    }

                    AsmInstrUnaryKind::Not => {}
                }
            }

            AsmInstr::Cmp { src, dst, .. } => {
                src.add_uses(&mut ud);
                dst.add_uses(&mut ud);

                ud.defs.insert(AsmLocation::Flags);
            }

            AsmInstr::Test { src, dst, .. } => {
                src.add_uses(&mut ud);
                dst.add_uses(&mut ud);

                ud.defs.insert(AsmLocation::Flags);
            }

            AsmInstr::Setcc { dst, .. } => {
                ud.uses.insert(AsmLocation::Flags);

                dst.add_uses(&mut ud);
                dst.add_defs(&mut ud);
            }

            AsmInstr::Jmpcc { .. } => {
                ud.uses.insert(AsmLocation::Flags);
            }

            AsmInstr::Shift { src, dst, .. } => {
                src.add_uses(&mut ud);

                dst.add_uses(&mut ud);
                dst.add_defs(&mut ud);

                ud.uses.insert(AsmLocation::Flags);
                ud.defs.insert(AsmLocation::Flags);
            }

            AsmInstr::Jmp { .. } => {}

            AsmInstr::Movsx { src, dst, .. } | AsmInstr::Movzx { src, dst, .. } => {
                src.add_uses(&mut ud);
                dst.add_defs(&mut ud);
            }

            AsmInstr::Lea { src, dst } => {
                src.add_uses(&mut ud);
                dst.add_defs(&mut ud);
            }

            AsmInstr::Cvttsd2si { src, dst, .. } | AsmInstr::Cvttss2si { src, dst, .. } => {
                src.add_uses(&mut ud);
                dst.add_defs(&mut ud);
            }

            AsmInstr::Cvtsi2sd { src, dst, .. }
            | AsmInstr::Cvtsi2ss { src, dst, .. }
            | AsmInstr::Cvtss2sd { src, dst }
            | AsmInstr::Cvtsd2ss { src, dst } => {
                src.add_uses(&mut ud);
                dst.add_defs(&mut ud);
            }

            AsmInstr::Cdq { asm_ty } => {
                ud.uses.insert(AsmLocation::Reg(AsmReg::Ax));

                let dst = match asm_ty {
                    AsmType::Byte => AsmReg::Ax,

                    AsmType::Word | AsmType::Long | AsmType::Quad => AsmReg::Dx,

                    _ => unreachable!(),
                };

                ud.defs.insert(AsmLocation::Reg(dst));
            }

            AsmInstr::Div { op, asm_ty } | AsmInstr::Idiv { op, asm_ty } => {
                op.add_uses(&mut ud);

                ud.uses.insert(AsmLocation::Reg(AsmReg::Ax));
                ud.defs.insert(AsmLocation::Reg(AsmReg::Ax));

                if !matches!(asm_ty, AsmType::Byte) {
                    ud.uses.insert(AsmLocation::Reg(AsmReg::Dx));
                    ud.defs.insert(AsmLocation::Reg(AsmReg::Dx));
                }

                ud.defs.insert(AsmLocation::Flags);
            }

            AsmInstr::Push(op) => {
                op.add_uses(&mut ud);

                ud.uses.insert(AsmLocation::Reg(AsmReg::Sp));
                ud.defs.insert(AsmLocation::Reg(AsmReg::Sp));
            }

            AsmInstr::Pop(op) => {
                ud.uses.insert(AsmLocation::Reg(AsmReg::Sp));
                ud.defs.insert(AsmLocation::Reg(AsmReg::Sp));

                op.add_defs(&mut ud);
            }

            AsmInstr::Ret => {
                for reg in [AsmReg::Sp, AsmReg::Ax, AsmReg::Dx, AsmReg::Cx] {
                    ud.uses.insert(AsmLocation::Reg(reg));
                }

                ud.defs.insert(AsmLocation::Reg(AsmReg::Sp));
            }

            AsmInstr::CallRt { target } => {
                use AsmReg::*;

                let args: &[AsmReg] = match target {
                    RtFn::Print => &[Di, Si, Dx],
                    RtFn::Call => &[Di, Si, Dx, Cx, R8],
                };

                for &reg in args {
                    ud.uses.insert(AsmLocation::Reg(reg));
                }

                ud.uses.insert(AsmLocation::Reg(R14));
                ud.uses.insert(AsmLocation::Reg(Sp));

                add_call_clobbers(&mut ud);
            }

            AsmInstr::Call { conv, .. } | AsmInstr::CallIndirect { conv, .. } => {
                use AsmReg::*;

                let args: &[AsmReg] = match conv {
                    CallConv::SystemV => &[
                        Di, Si, Dx, Cx, R8, R9, Xmm0, Xmm1, Xmm2, Xmm3, Xmm4, Xmm5, Xmm6, Xmm7,
                    ],

                    CallConv::Jit => &[Di, R13, R14, R15],
                };

                for &reg in args {
                    ud.uses.insert(AsmLocation::Reg(reg));
                }

                if let AsmInstr::CallIndirect { target, .. } = self {
                    ud.uses.insert(AsmLocation::Reg(*target));
                }

                ud.uses.insert(AsmLocation::Reg(Sp));

                add_call_clobbers(&mut ud);
            }

            AsmInstr::Label(_) | AsmInstr::Newline | AsmInstr::Comment(_) => {}
        }

        ud
    }
}

fn add_call_clobbers(ud: &mut UseDef<AsmLocation>) {
    use AsmReg::*;

    let caller_saved = [
        Ax, Cx, Dx, Si, Di, R8, R9, R10, R11, Xmm0, Xmm1, Xmm2, Xmm3, Xmm4, Xmm5, Xmm6, Xmm7, Xmm15,
    ];

    for reg in caller_saved {
        ud.defs.insert(AsmLocation::Reg(reg));
    }

    ud.defs.insert(AsmLocation::Flags);
}

impl AsmInstr {
    fn rewrite_spilled(&mut self, slots: &BTreeMap<String, isize>) {
        for operand in self.operands_mut().into_iter().flatten() {
            if let AsmOp::Pseudo { name, .. } = operand
                && let Some(&offset) = slots.get(name) {
                    *operand = AsmOp::Mem(AsmReg::Bp, offset);
                }
        }
    }

    fn rewrite_colored(&mut self, assignments: &BTreeMap<String, AsmReg>) {
        for operand in self.operands_mut().into_iter().flatten() {
            if let AsmOp::Pseudo { name, .. } = operand
                && let Some(&reg) = assignments.get(name) {
                    *operand = AsmOp::Reg(reg);
                }
        }
    }

    fn operands_mut(&mut self) -> [Option<&mut AsmOp>; 2] {
        match self {
            Self::Mov { src, dst, .. }
            | Self::Movsx { src, dst, .. }
            | Self::Movzx { src, dst, .. }
            | Self::Lea { src, dst }
            | Self::Cmp { src, dst, .. }
            | Self::Test { src, dst, .. }
            | Self::Shift { src, dst, .. }
            | Self::Cvtsi2sd { src, dst, .. }
            | Self::Cvttsd2si { src, dst, .. }
            | Self::Cvtsi2ss { src, dst, .. }
            | Self::Cvttss2si { src, dst, .. }
            | Self::Cvtss2sd { src, dst }
            | Self::Cvtsd2ss { src, dst } => [Some(src), Some(dst)],

            Self::Bin { lhs, rhs, .. } => [Some(lhs), Some(rhs)],

            Self::Unary { dst, .. } | Self::Setcc { dst, .. } => [Some(dst), None],

            Self::Push(op) | Self::Pop(op) | Self::Div { op, .. } | Self::Idiv { op, .. } => {
                [Some(op), None]
            }

            Self::Ret
            | Self::Cdq { .. }
            | Self::Jmp { .. }
            | Self::Jmpcc { .. }
            | Self::Label(_)
            | Self::Call { .. }
            | Self::CallIndirect { .. }
            | Self::CallRt { .. }
            | Self::Comment(_)
            | Self::Newline => [None, None],
        }
    }

    fn operands(&self) -> [Option<&AsmOp>; 2] {
        match self {
            Self::Mov { src, dst, .. }
            | Self::Movsx { src, dst, .. }
            | Self::Movzx { src, dst, .. }
            | Self::Lea { src, dst }
            | Self::Cmp { src, dst, .. }
            | Self::Test { src, dst, .. }
            | Self::Shift { src, dst, .. }
            | Self::Cvtsi2sd { src, dst, .. }
            | Self::Cvttsd2si { src, dst, .. }
            | Self::Cvtsi2ss { src, dst, .. }
            | Self::Cvttss2si { src, dst, .. }
            | Self::Cvtss2sd { src, dst }
            | Self::Cvtsd2ss { src, dst } => [Some(src), Some(dst)],

            Self::Bin { lhs, rhs, .. } => [Some(lhs), Some(rhs)],

            Self::Unary { dst, .. } | Self::Setcc { dst, .. } => [Some(dst), None],

            Self::Push(op) | Self::Pop(op) | Self::Div { op, .. } | Self::Idiv { op, .. } => {
                [Some(op), None]
            }

            Self::Ret
            | Self::Cdq { .. }
            | Self::Jmp { .. }
            | Self::Jmpcc { .. }
            | Self::Label(_)
            | Self::Call { .. }
            | Self::CallIndirect { .. }
            | Self::CallRt { .. }
            | Self::Comment(_)
            | Self::Newline => [None, None],
        }
    }
}

fn assign_spill_slots(
    spilled: &BTreeSet<String>,
    sizes: &BTreeMap<String, usize>,
    existing_space: usize,
) -> (BTreeMap<String, isize>, usize) {
    let mut used = existing_space.div_ceil(16) * 16;
    let mut slots = BTreeMap::new();

    for name in spilled {
        let size = *sizes.get(name).expect("Missing size for spilled pseudo");
        assert!(size.is_power_of_two(), "Unsupported spill-slot size: {size}");

        // Align the beginning of each variable's slot, not every slot to 8.
        used = used.div_ceil(size) * size;
        used += size;
        slots.insert(name.clone(), -(used as isize));
    }

    // The frame stays 16-byte aligned, even though its slots are packed.
    let space = used.div_ceil(16) * 16;

    (slots, space)
}

fn collect_reg_classes(instructions: &[AsmInstr]) -> Result<BTreeMap<String, RegClass>, String> {
    let mut classes = BTreeMap::new();

    for instr in instructions {
        for operand in instr.operands().into_iter().flatten() {
            let AsmOp::Pseudo { name, ty } = operand else {
                continue;
            };

            let class = RegClass::of_type(ty);

            match classes.get(name) {
                Some(existing) if *existing != class => {
                    return Err(format!(
                        "Pseudo-register `{name}` has conflicting \
                         register classes: {existing:?} and {class:?}"
                    ));
                }

                None => {
                    classes.insert(name.clone(), class);
                }

                _ => {}
            }
        }
    }

    Ok(classes)
}

fn collect_spill_costs(instructions: &[AsmInstr]) -> BTreeMap<String, usize> {
    let mut costs = BTreeMap::new();

    for instr in instructions {
        let ud = instr.use_def();

        for location in ud.uses.iter().chain(ud.defs.iter()) {
            if let AsmLocation::Pseudo(name) = location {
                *costs.entry(name.clone()).or_insert(0) += 1;
            }
        }
    }

    costs
}

fn is_redundant_move(instr: &AsmInstr) -> bool {
    match instr {
        AsmInstr::Mov {
            src: AsmOp::Reg(src),
            dst: AsmOp::Reg(dst),
            asm_ty,
        } => {
            // A 32-bit GP move also zeroes the upper 32 bits.
            // Conservatively retain all Long self-moves.
            src == dst && *asm_ty != AsmType::Long
        }

        AsmInstr::Mov {
            src: AsmOp::Mem(src_base, src_offset),
            dst: AsmOp::Mem(dst_base, dst_offset),
            ..
        } => src_base == dst_base && src_offset == dst_offset,

        _ => false,
    }
}

fn collect_spill_sizes(
    instructions: &[AsmInstr],
    representative: &BTreeMap<String, InterferenceNode>,
) -> BTreeMap<String, usize> {
    let mut sizes = BTreeMap::<String, usize>::new();

    for instr in instructions {
        for operand in instr.operands().into_iter().flatten() {
            if let AsmOp::Pseudo { name, ty } = operand
                && let Some(InterferenceNode::Pseudo(rep)) = representative.get(name)
            {
                let size = (*ty).size_of();
                sizes
                    .entry(rep.clone())
                    .and_modify(|existing| *existing = (*existing).max(size))
                    .or_insert(size);
            }
        }
    }

    sizes
}

