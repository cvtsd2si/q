pub mod error;

mod coalescing;
mod interference;

use self::{
    coalescing::{Coalescer, collect_moves, collect_precolored_moves},
    error::RegAllocError,
    interference::{InterferenceGraph, InterferenceNode},
};
use crate::{
    analysis::{
        cfg::Cfg,
        liveness::{Liveness, LivenessInstruction, UseDef},
    },
    asm::{
        abi::{
            CALLER_SAVED, GP_PUSH_BYTES, JIT_RETURN_KIND, JIT_RETURN_PAYLOAD, JIT_RETURN_TAG,
            JIT_RT, SSE_CALL_SAVE_TY, SYSV_STACK_ALIGNMENT, call_live_in_regs, needs_caller_save,
            rt_arg_regs,
        },
        r#gen::{AsmFunc, AsmInstr, AsmInstrBinKind, AsmInstrUnaryKind, AsmOp, AsmReg, AsmType},
    },
    parser::ast::Type,
};
use std::collections::{BTreeMap, BTreeSet};

pub trait Regalloc {
    fn regalloc(self) -> Result<AsmFunc, RegAllocError>;
}

impl Regalloc for AsmFunc {
    fn regalloc(mut self) -> Result<AsmFunc, RegAllocError> {
        /* Build the control-flow graph.  */
        let cfg = Cfg::build(&self.body).map_err(|e| RegAllocError { msg: e })?;

        /* Compute liveness.  */
        let liveness = Liveness::<AsmLocation>::analyze(&self.body, &cfg);

        /* Generate a Vec which is filled up with a BTreeSet<String> for each asm instr.
         * The BTreeSet<String> represents a collection of pseudo regs which must surive
         * that instruction.
         *
         * NOTE: This is populated only for call instrs.  */
        let live_through_calls: Vec<BTreeSet<String>> = self
            .body
            .iter()
            .enumerate()
            .map(|(idx, instr)| {
                if !instr.is_call() {
                    return BTreeSet::new();
                }

                /* live_in contains the values which are needed when
                 * the instruction begins.  */
                let live_in = &liveness.instrs[idx].live_in;
                /* live_out contains the values which are needed when
                 * after the instructions.  */
                let live_out = &liveness.instrs[idx].live_out;

                /* live_in n live_out
                 *
                 * If the value happens to be in the both sets, then
                 * this means it must survive the call.  */
                live_in
                    .iter()
                    .filter_map(|x| match x {
                        AsmLocation::Pseudo(name) if live_out.contains(x) => Some(name.clone()),
                        _ => None,
                    })
                    .collect()
            })
            .collect();

        /* Build the interference graph.  */
        let graph = InterferenceGraph::build(&self.body, &liveness);

        /* For each pseudo reg, figure out which register class it belongs to.  */
        let reg_classes = collect_reg_classes(&self.body).map_err(|e| RegAllocError { msg: e })?;

        /* Collect both pseudo-to-pseudo and pseudo-to-physical
         * (and vice-versa), aka "precolored" move candidates once.  */
        let moves = collect_moves(&self.body);
        let precolored_moves = collect_precolored_moves(&self.body);

        /* Coalesce pseudo-registers.  */
        let mut coalescing = Coalescer::new(graph, &reg_classes);

        loop {
            let mut changed = false;

            /* Attempt pseudo-to-pseudo coalescing.
             *
             * NOTE: `try_coalesce()` also calls `try_coalesce_precolored()`
             *       so if the move is eligible, it might end up coalesced here.  */
            for mov in &moves {
                changed |= coalescing.try_coalesce(mov);
            }

            /* Attempt pseudo-to-precolored coalescing.  */
            for (name, reg) in &precolored_moves {
                changed |= coalescing.try_coalesce_precolored(name, *reg);
            }

            if !changed {
                break;
            }
        }

        /* Collect spill costs for original pseudo registers.  */
        let original_costs = collect_spill_costs(&self.body);

        /* Combine the spill costs of pseudo registers that were
         * merged during coalescing.
         *
         * NOTE: This is necessary because after coalescing, multiple
         * pseudo registers could be represented by a single node in
         * the interference graph.  */
        let mut spill_costs = BTreeMap::new();

        for (name, cost) in original_costs {
            if let Some(rep) = coalescing.reps.get(&name) {
                if let InterferenceNode::Pseudo(rep) = rep {
                    *spill_costs.entry(rep.clone()).or_insert(0) += cost;
                }
            }
        }

        /* Color the coalesced graph.  */
        let (colored, spilled) = coalescing.graph.color(&reg_classes, &spill_costs);

        /* Assign stack slots to spilled representatives.  */
        let spill_sizes = collect_spill_sizes(&self.body, &coalescing.reps);
        let (spill_slots, space) = assign_spill_slots(&spilled, &spill_sizes, self.space);

        /* Restore locations for every original pseudo.  */
        let mut assignments = BTreeMap::new();
        let mut slots = BTreeMap::new();

        for (name, representative) in &coalescing.reps {
            match representative {
                /* This pseudo was merged into a physical register.  */
                InterferenceNode::Reg(reg) => {
                    assignments.insert(name.clone(), *reg);
                }

                /* This pseudo belongs to a surviving pseudo node.  */
                InterferenceNode::Pseudo(rep) => {
                    /* The representative was successfully colored.  */
                    if let Some(&reg) = colored.get(rep) {
                        assignments.insert(name.clone(), reg);
                    /* The representative was spilled to the stack.  */
                    } else if let Some(&offset) = spill_slots.get(rep) {
                        slots.insert(name.clone(), offset);
                    } else {
                        panic!("No allocation for {name}, representative {rep}"); // FIXME: don't panic
                    }
                }
            }
        }

        /* Rewrite assembly operands.  */
        for instr in &mut self.body {
            instr.rewrite_colored(&assignments);
            instr.rewrite_spilled(&slots);
        }

        /* Insert caller-save / caller-restore instructions. */
        let old_body = std::mem::take(&mut self.body);

        let mut new_body = Vec::new();

        for (idx, instr) in old_body.into_iter().enumerate() {
            if !instr.is_call() {
                new_body.push(instr);
                continue;
            }

            /* Get pseudo-registers live across this call.  */
            let live = &live_through_calls[idx];

            /* Convert pseudo-registers to physical registers.
             *
             * Spilled values don't need saving here because
             * they're already stored in the stack frame.  */
            let regs: Vec<AsmReg> = live
                .iter()
                .filter_map(|name| assignments.get(name).copied())
                .filter(|reg| needs_caller_save(*reg))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();

            /* Generate the preservation sequences.  */
            let (save, restore) = call_save_restore(&regs);

            /* Save registers.  */
            new_body.extend(save);

            /* Execute the original call.  */
            new_body.push(instr);

            /* Restore registers.  */
            new_body.extend(restore);
        }

        self.body = new_body;

        /* Remove moves between identical physical locations.  */
        self.body.retain(|instr| !is_redundant_move(instr));

        /*. Update the required stack-frame size.  */
        self.space = space;

        /* Assert there are no pseudo-registers left.  */
        for instr in &self.body {
            for operand in instr.operands().into_iter().flatten() {
                assert!(
                    !matches!(operand, AsmOp::Pseudo { .. }),
                    "Unallocated pseudo-register in {instr:?}"
                );
            }
        }

        Ok(self)
    }
}

/// Iterate through the instructions, and for each individual instruction, gather up the pseudo
/// registers it contains, and determine their register classes.
fn collect_reg_classes(instructions: &[AsmInstr]) -> Result<BTreeMap<String, RegClass>, String> {
    let mut classes = BTreeMap::new();

    for instr in instructions {
        for operand in instr.operands().into_iter().flatten() {
            if let AsmOp::Pseudo { name, ty } = operand {
                classes.insert(name.clone(), RegClass::of_type(ty));
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

fn assign_spill_slots(
    spilled: &BTreeSet<String>,
    sizes: &BTreeMap<String, usize>,
    existing_space: usize,
) -> (BTreeMap<String, isize>, usize) {
    let mut used = existing_space.div_ceil(SYSV_STACK_ALIGNMENT) * SYSV_STACK_ALIGNMENT;
    let mut slots = BTreeMap::new();

    for name in spilled {
        let size = *sizes.get(name).expect("Missing size for spilled pseudo");
        assert!(
            size.is_power_of_two(),
            "Unsupported spill-slot size: {size}"
        );

        // Align the beginning of each variable's slot, not every slot to 8.
        used = used.div_ceil(size) * size;
        used += size;
        slots.insert(name.clone(), -(used as isize));
    }

    // The frame stays SYSV_STACK_ALIGNMENT-byte aligned, even though its slots are packed.
    let space = used.div_ceil(SYSV_STACK_ALIGNMENT) * SYSV_STACK_ALIGNMENT;

    (slots, space)
}

fn call_save_restore(regs: &[AsmReg]) -> (Vec<AsmInstr>, Vec<AsmInstr>) {
    let mut gp = Vec::new();
    let mut sse = Vec::new();

    for &reg in regs {
        if reg.is_xmm() {
            sse.push(reg);
        } else {
            gp.push(reg);
        }
    }

    /* Each GP push consumes `GP_PUSH_BYTES`.  */
    let gp_bytes = gp.len() * GP_PUSH_BYTES;

    /* Each scalar SSE save consumes `SSE_CALL_SAVE_TY.size()` bytes.  */
    let sse_bytes = sse.len() * SSE_CALL_SAVE_TY.size();

    /* Ensure the total stack adjustment is a multiple of `SYSV_STACK_ALIGNMENT`.  */
    let total = (gp_bytes + sse_bytes).div_ceil(SYSV_STACK_ALIGNMENT) * SYSV_STACK_ALIGNMENT;

    /* GP registers are saved using push.
     * SSE registers and alignment padding use sub %rsp.  */
    let stack_bytes = total - gp_bytes;

    let mut save = Vec::new();
    let mut restore = Vec::new();

    /* Save GP registers.  */
    for &reg in &gp {
        save.push(AsmInstr::Push(AsmOp::Reg(reg)));
    }

    /* Reserve SSE slots and alignment padding.  */
    if stack_bytes != 0 {
        save.push(AsmInstr::Bin {
            kind: AsmInstrBinKind::Sub,
            lhs: AsmOp::Imm(stack_bytes as i64),
            rhs: AsmOp::Reg(AsmReg::Sp),
            asm_ty: AsmType::Quad,
        });
    }

    /* Save XMM registers.  */
    for (idx, &reg) in sse.iter().enumerate() {
        save.push(AsmInstr::Mov {
            src: AsmOp::Reg(reg),
            dst: AsmOp::Mem(AsmReg::Sp, (idx * 8) as isize),
            asm_ty: SSE_CALL_SAVE_TY,
        });
    }

    /* Function call here.  */

    /* Restore XMM registers.  */
    for (idx, &reg) in sse.iter().enumerate().rev() {
        restore.push(AsmInstr::Mov {
            src: AsmOp::Mem(AsmReg::Sp, (idx * 8) as isize),
            dst: AsmOp::Reg(reg),
            asm_ty: AsmType::Double,
        });
    }

    /* Release SSE slots and alignment padding.  */
    if stack_bytes != 0 {
        restore.push(AsmInstr::Bin {
            kind: AsmInstrBinKind::Add,
            lhs: AsmOp::Imm(stack_bytes as i64),
            rhs: AsmOp::Reg(AsmReg::Sp),
            asm_ty: AsmType::Quad,
        });
    }

    /* Restore GP registers in reverse order.  */
    for &reg in gp.iter().rev() {
        restore.push(AsmInstr::Pop(AsmOp::Reg(reg)));
    }

    (save, restore)
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
                for reg in [
                    AsmReg::Sp,
                    JIT_RETURN_PAYLOAD,
                    JIT_RETURN_TAG,
                    JIT_RETURN_KIND,
                ] {
                    ud.uses.insert(AsmLocation::Reg(reg));
                }

                ud.defs.insert(AsmLocation::Reg(AsmReg::Sp));
            }

            AsmInstr::CallRt { target } => {
                for reg in rt_arg_regs(*target) {
                    ud.uses.insert(AsmLocation::Reg(reg));
                }

                ud.uses.insert(AsmLocation::Reg(JIT_RT));
                ud.uses.insert(AsmLocation::Reg(AsmReg::Sp));

                add_call_clobbers(&mut ud);
            }

            AsmInstr::Call { conv, .. } | AsmInstr::CallIndirect { conv, .. } => {
                for reg in call_live_in_regs(*conv) {
                    ud.uses.insert(AsmLocation::Reg(reg));
                }

                if let AsmInstr::CallIndirect { target, .. } = self {
                    ud.uses.insert(AsmLocation::Reg(*target));
                }

                ud.uses.insert(AsmLocation::Reg(AsmReg::Sp));

                add_call_clobbers(&mut ud);
            }

            AsmInstr::Label(_) | AsmInstr::Newline | AsmInstr::Comment(_) => {}
        }

        ud
    }
}

fn add_call_clobbers(ud: &mut UseDef<AsmLocation>) {
    for &reg in CALLER_SAVED {
        if !needs_caller_save(reg) {
            ud.defs.insert(AsmLocation::Reg(reg));
        }
    }

    ud.defs.insert(AsmLocation::Flags);
}

impl AsmInstr {
    fn rewrite_spilled(&mut self, slots: &BTreeMap<String, isize>) {
        for operand in self.operands_mut().into_iter().flatten() {
            if let AsmOp::Pseudo { name, .. } = operand
                && let Some(&offset) = slots.get(name)
            {
                *operand = AsmOp::Mem(AsmReg::Bp, offset);
            }
        }
    }

    fn rewrite_colored(&mut self, assignments: &BTreeMap<String, AsmReg>) {
        for operand in self.operands_mut().into_iter().flatten() {
            if let AsmOp::Pseudo { name, .. } = operand
                && let Some(&reg) = assignments.get(name)
            {
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

    fn is_call(&self) -> bool {
        matches!(
            self,
            AsmInstr::Call { .. } | AsmInstr::CallIndirect { .. } | AsmInstr::CallRt { .. }
        )
    }
}
