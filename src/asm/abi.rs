use crate::asm::r#gen::{AsmReg, AsmType, CallConv, RtFn};

pub const SYSV_STACK_ALIGNMENT: usize = 16;

/// Bytes occupied by a 64-bit pushq instruction.
pub const GP_PUSH_BYTES: usize = 8;

/// Save the low 64 bits of caller-saved XMM registers.
///
/// The allocator currently supports scalar f32/f64 values,
/// so an 8-byte movsd save preserves the live scalar value.
pub const SSE_CALL_SAVE_TY: AsmType = AsmType::Double;

pub const GP_ALLOCATABLE: &[AsmReg] = &[AsmReg::R8, AsmReg::R9];

pub const SSE_ALLOCATABLE: &[AsmReg] = &[
    AsmReg::Xmm0,
    AsmReg::Xmm1,
    AsmReg::Xmm2,
    AsmReg::Xmm3,
    AsmReg::Xmm4,
    AsmReg::Xmm5,
    AsmReg::Xmm6,
    AsmReg::Xmm7,
];

pub const CALLER_SAVED: &[AsmReg] = &[
    AsmReg::Ax,
    AsmReg::Cx,
    AsmReg::Dx,
    AsmReg::Si,
    AsmReg::Di,
    AsmReg::R8,
    AsmReg::R9,
    AsmReg::R10,
    AsmReg::R11,
    AsmReg::Xmm0,
    AsmReg::Xmm1,
    AsmReg::Xmm2,
    AsmReg::Xmm3,
    AsmReg::Xmm4,
    AsmReg::Xmm5,
    AsmReg::Xmm6,
    AsmReg::Xmm7,
    AsmReg::Xmm14,
    AsmReg::Xmm15,
];

pub const SYSV_GP_ARG_REGS: &[AsmReg] = &[
    AsmReg::Di,
    AsmReg::Si,
    AsmReg::Dx,
    AsmReg::Cx,
    AsmReg::R8,
    AsmReg::R9,
];

pub const SYSV_SSE_ARG_REGS: &[AsmReg] = &[
    AsmReg::Xmm0,
    AsmReg::Xmm1,
    AsmReg::Xmm2,
    AsmReg::Xmm3,
    AsmReg::Xmm4,
    AsmReg::Xmm5,
    AsmReg::Xmm6,
    AsmReg::Xmm7,
];

pub const JIT_CALL_LIVE_INS: &[AsmReg] = &[JIT_ARGS, JIT_VM, JIT_RT, JIT_ENTRYPOINTS];

pub const GP_SCRATCH_1: AsmReg = AsmReg::R10;
pub const GP_SCRATCH_2: AsmReg = AsmReg::R11;

pub const SSE_SCRATCH_1: AsmReg = AsmReg::Xmm14;
pub const SSE_SCRATCH_2: AsmReg = AsmReg::Xmm15;

pub const JIT_ARGS: AsmReg = AsmReg::Di;
pub const JIT_VM: AsmReg = AsmReg::R13;
pub const JIT_RT: AsmReg = AsmReg::R14;
pub const JIT_ENTRYPOINTS: AsmReg = AsmReg::R15;

pub const ENTRY_RESULT_OUT_ARG: AsmReg = SYSV_GP_ARG_REGS[0];
pub const ENTRY_VM_ARG: AsmReg = SYSV_GP_ARG_REGS[1];
pub const ENTRY_RT_ARG: AsmReg = SYSV_GP_ARG_REGS[2];
pub const ENTRY_ARGS_ARG: AsmReg = SYSV_GP_ARG_REGS[3];
pub const ENTRY_BODY_ARG: AsmReg = SYSV_GP_ARG_REGS[4];
pub const ENTRY_END_PC_ARG: AsmReg = SYSV_GP_ARG_REGS[5];

pub const ENTRY_ENTRYPOINTS_RBP_OFFSET: isize = 16;

pub const JIT_RESULT_STORAGE: AsmReg = AsmReg::R12;

pub const JIT_RETURN_PAYLOAD: AsmReg = SYSV_GP_RETURN;
pub const JIT_RETURN_TAG: AsmReg = AsmReg::Dx;
pub const JIT_RETURN_KIND: AsmReg = AsmReg::Cx;

pub const JIT_ENTRY_PRESERVED: &[AsmReg] = &[JIT_RESULT_STORAGE, JIT_VM, JIT_RT, JIT_ENTRYPOINTS];

pub const SYSV_GP_RETURN: AsmReg = AsmReg::Ax;

// Register used to load an indirect runtime-call target.
// Must not overlap with the runtime function's argument registers.
pub const RT_CALL_SCRATCH: AsmReg = GP_SCRATCH_1;

pub fn is_caller_saved(reg: AsmReg) -> bool {
    CALLER_SAVED.contains(&reg)
}

pub fn is_allocatable(reg: AsmReg) -> bool {
    GP_ALLOCATABLE.contains(&reg) || SSE_ALLOCATABLE.contains(&reg)
}

pub fn needs_caller_save(reg: AsmReg) -> bool {
    is_allocatable(reg) && is_caller_saved(reg)
}

pub fn call_live_in_regs(conv: CallConv) -> impl Iterator<Item = AsmReg> {
    let (gp, sse) = match conv {
        CallConv::SystemV => (SYSV_GP_ARG_REGS, SYSV_SSE_ARG_REGS),
        CallConv::Jit => (JIT_CALL_LIVE_INS, &[][..]),
    };

    gp.iter().chain(sse.iter()).copied()
}

pub fn rt_arg_regs(target: RtFn) -> impl Iterator<Item = AsmReg> {
    let count = match target {
        RtFn::Print => 3,
        RtFn::Call => 5,
    };

    SYSV_GP_ARG_REGS.iter().copied().take(count)
}

pub fn rt_arg_reg(target: RtFn, index: usize) -> AsmReg {
    rt_arg_regs(target).nth(index).unwrap()
}
