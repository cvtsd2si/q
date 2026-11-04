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
    codegen::asm::{
        abi::{
            amd64::{
                CALLER_SAVED, GP_PUSH_BYTES, SSE_CALL_SAVE_TY, SYSV_STACK_ALIGNMENT,
                needs_callee_save, needs_caller_save, rt_arg_regs,
            },
            call_live_in_regs,
            internal::{JIT_RETURN_KIND, JIT_RETURN_PAYLOAD, JIT_RETURN_TAG, JIT_RT},
        },
        r#gen::{AsmFunc, AsmInstr, AsmInstrBinKind, AsmInstrUnaryKind, AsmOp, AsmReg, AsmType},
    },
    parser::ast::Type,
    parser::ast::TypeInterner,
};
use std::collections::{BTreeMap, BTreeSet};

pub trait Regalloc {
    fn regalloc(self, types: &TypeInterner) -> Result<AsmFunc, RegAllocError>;
}

impl Regalloc for AsmFunc {
    fn regalloc(mut self, types: &TypeInterner) -> Result<AsmFunc, RegAllocError> {
        let allocation = Allocation::compute(&self, types)?;

        allocation.rewrite(&mut self.body);
        allocation.preserve_callers(&mut self.body);

        self.body.retain(|instr| !is_redundant_move(instr));

        allocation.preserve_callees(&mut self.body);

        self.space = allocation.frame_space;

        Ok(self)
    }
}

struct Allocation<'types> {
    /* Maps pseudo-registers to physical registers.  */
    registers: BTreeMap<String, AsmReg>,
    /* Maps spilled pseudo-registers to stack offsets.  */
    spill_slots: BTreeMap<String, isize>,
    /* Which callee-saved registers require preservation?  */
    callee_saved: BTreeSet<AsmReg>,
    /* Which pseudo-registers must survive a particular instruction?
     * For each asm instr, the Vec contains a BTreeSet containing the
     * pseudo-registers that must survive the instruction at that idx.  */
    live_across_calls: Vec<BTreeSet<String>>,
    /* The final, correctly aligned allocation size.  */
    frame_space: usize,

    types: &'types TypeInterner,
}

impl<'types> Allocation<'types> {
    fn compute(func: &AsmFunc, types: &'types TypeInterner) -> Result<Self, RegAllocError> {
        let body = &func.body;

        let cfg = Cfg::build(body).map_err(|msg| RegAllocError { msg })?;

        let liveness = Liveness::<AsmLocation>::analyze(body, &cfg);

        let live_across_calls = find_live_across_calls(body, &liveness);

        let classes = collect_reg_classes(body).map_err(|msg| RegAllocError { msg })?;

        let graph = InterferenceGraph::build(body, &liveness);
        let mut coalescer = Coalescer::new(graph, &classes);

        let moves = collect_moves(body);
        let precolored_moves = collect_precolored_moves(body);

        loop {
            let mut changed = false;

            for mov in &moves {
                changed |= coalescer.try_coalesce(mov);
            }

            for (name, reg) in &precolored_moves {
                changed |= coalescer.try_coalesce_precolored(name, *reg);
            }

            if !changed {
                break;
            }
        }

        let mut spill_costs = BTreeMap::new();

        for (name, cost) in collect_spill_costs(body) {
            if let Some(InterferenceNode::Pseudo(rep)) = coalescer.reps.get(&name) {
                *spill_costs.entry(rep.clone()).or_insert(0) += cost;
            }
        }

        let (colored, spilled) = coalescer.graph.color(&classes, &spill_costs);

        let mut allocation = Self {
            registers: BTreeMap::new(),
            spill_slots: BTreeMap::new(),
            callee_saved: BTreeSet::new(),
            live_across_calls,
            frame_space: func.space,
            types,
        };

        for (name, representative) in &coalescer.reps {
            let reg = match representative {
                InterferenceNode::Reg(reg) => Some(*reg),
                InterferenceNode::Pseudo(rep) => colored.get(rep).copied(),
            };

            if let Some(reg) = reg {
                allocation.registers.insert(name.clone(), reg);
            }
        }

        allocation.find_callee_saved(body);
        allocation.assign_spills(func, &coalescer, &spilled);

        Ok(allocation)
    }

    fn find_callee_saved(&mut self, body: &[AsmInstr]) {
        self.callee_saved = body
            .iter()
            .flat_map(|instr| instr.use_def().defs)
            .filter_map(|location| match location {
                AsmLocation::Reg(reg) => Some(reg),
                AsmLocation::Pseudo(name) => self.registers.get(&name).copied(),
                AsmLocation::Flags => None,
            })
            .filter(|&reg| needs_callee_save(reg))
            .collect()
    }

    fn assign_spills(&mut self, func: &AsmFunc, coalescer: &Coalescer, spilled: &BTreeSet<String>) {
        let sizes = collect_spill_sizes(&func.body, &coalescer.reps, self.types);

        let mut used = func.space.div_ceil(SYSV_STACK_ALIGNMENT) * SYSV_STACK_ALIGNMENT;

        let mut representative_slots = BTreeMap::new();

        for name in spilled {
            let size = *sizes.get(name).unwrap();

            used = used.div_ceil(size) * size;
            used += size;

            representative_slots.insert(name.clone(), -(used as isize));
        }

        for (name, representative) in &coalescer.reps {
            if self.registers.contains_key(name) {
                continue;
            }

            let InterferenceNode::Pseudo(rep) = representative else {
                unreachable!("Unallocated precolored register");
            };

            let offset = representative_slots.get(rep).expect("Missing spill slot");

            self.spill_slots.insert(name.clone(), *offset);
        }

        let push_bytes = self.callee_saved.len() * GP_PUSH_BYTES;

        self.frame_space =
            (used + push_bytes).div_ceil(SYSV_STACK_ALIGNMENT) * SYSV_STACK_ALIGNMENT - push_bytes;
    }

    fn rewrite(&self, body: &mut [AsmInstr]) {
        for instr in body {
            instr.rewrite_colored(&self.registers);
            instr.rewrite_spilled(&self.spill_slots);
        }
    }

    fn preserve_callers(&self, body: &mut Vec<AsmInstr>) {
        let old_body = std::mem::take(body);

        for (idx, instr) in old_body.into_iter().enumerate() {
            if !instr.is_call() {
                body.push(instr);
                continue;
            }

            let regs: Vec<_> = self.live_across_calls[idx]
                .iter()
                .filter_map(|name| self.registers.get(name).copied())
                .filter(|reg| needs_caller_save(*reg))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();

            let (save, restore) = call_save_restore(&regs);

            body.extend(save);
            body.push(instr);
            body.extend(restore);
        }
    }

    fn preserve_callees(&self, body: &mut Vec<AsmInstr>) {
        if self.callee_saved.is_empty() {
            return;
        }

        let idx = find_prologue_idx(body);

        body.splice(
            idx + 1..idx + 1,
            self.callee_saved
                .iter()
                .copied()
                .map(|reg| AsmInstr::Push(AsmOp::Reg(reg))),
        );

        for idx in (0..body.len()).rev() {
            let is_epilogue = matches!(
                &body[idx],
                AsmInstr::Mov {
                    src: AsmOp::Reg(AsmReg::Bp),
                    dst: AsmOp::Reg(AsmReg::Sp),
                    asm_ty: AsmType::Quad,
                }
            );

            if is_epilogue {
                body.splice(
                    idx..idx,
                    self.callee_saved
                        .iter()
                        .rev()
                        .copied()
                        .map(|reg| AsmInstr::Pop(AsmOp::Reg(reg))),
                );
            }
        }
    }
}

fn find_live_across_calls(
    body: &[AsmInstr],
    liveness: &Liveness<AsmLocation>,
) -> Vec<BTreeSet<String>> {
    body.iter()
        .zip(&liveness.instrs)
        .map(|(instr, live)| {
            if !instr.is_call() {
                return BTreeSet::new();
            }

            live.live_in
                .intersection(&live.live_out)
                .filter_map(|location| match location {
                    AsmLocation::Pseudo(name) => Some(name.clone()),
                    _ => None,
                })
                .collect()
        })
        .collect()
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
    types: &TypeInterner,
) -> BTreeMap<String, usize> {
    let mut sizes = BTreeMap::<String, usize>::new();

    for instr in instructions {
        for operand in instr.operands().into_iter().flatten() {
            if let AsmOp::Pseudo { name, ty } = operand
                && let Some(InterferenceNode::Pseudo(rep)) = representative.get(name)
            {
                let size = (*ty).size_of(types);
                sizes
                    .entry(rep.clone())
                    .and_modify(|existing| *existing = (*existing).max(size))
                    .or_insert(size);
            }
        }
    }

    sizes
}

fn find_prologue_idx(body: &[AsmInstr]) -> usize {
    body.iter()
        .position(|instr| {
            matches!(
                instr,
                AsmInstr::Bin {
                    kind: AsmInstrBinKind::Sub,
                    lhs: AsmOp::Imm(0),
                    rhs: AsmOp::Reg(AsmReg::Sp),
                    asm_ty: AsmType::Quad,
                }
            )
        })
        .unwrap()
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
            Self::Indexed { base, index, .. } => {
                ud.uses.insert(AsmLocation::Reg(*base));
                ud.uses.insert(AsmLocation::Reg(*index));
            }
            Self::Imm(_) | Self::Data(_, _) | Self::PseudoMem(_, _, _) | Self::StaticAddr(_, _) => {
            }
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
            Self::Indexed { base, index, .. } => {
                ud.defs.insert(AsmLocation::Reg(*base));
                ud.defs.insert(AsmLocation::Reg(*index));
            }
            Self::Mem(reg, _) => {
                ud.uses.insert(AsmLocation::Reg(*reg));
            }
            Self::Imm(_) | Self::Data(_, _) | Self::PseudoMem(_, _, _) | Self::StaticAddr(_, _) => {
            }
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

    pub fn operands_mut(&mut self) -> [Option<&mut AsmOp>; 2] {
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
