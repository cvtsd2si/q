use crate::asm::abi::amd64::{GP_SCRATCH_1, SYSV_GP_ARG_REGS, SYSV_GP_RETURN};
use crate::asm::r#gen::AsmReg;

/// Register used to hold the pointer to the argument array in the internal ABI.
pub const JIT_ARGS: AsmReg = AsmReg::Di;

/// Register used to hold the pointer to the JitResult in the internal ABI.
pub const JIT_RESULT_STORAGE: AsmReg = AsmReg::R12;

/// Register used to hold the *mut Vm ptr in the internal ABI.
pub const JIT_VM: AsmReg = AsmReg::R13;

/// Register used to hold the *const Rt ptr in the internal ABI.
pub const JIT_RT: AsmReg = AsmReg::R14;

/// Register used to hold the ptr to `native_entrypoints` in the internal ABI.
pub const JIT_ENTRYPOINTS: AsmReg = AsmReg::R15;

/// Register used to hold the JitResult return payload.
pub const JIT_RETURN_PAYLOAD: AsmReg = SYSV_GP_RETURN;

/// Register used to hold the JitResult return tag.
pub const JIT_RETURN_TAG: AsmReg = AsmReg::Dx;

/// Register used to hold the numeric return kind.
pub const JIT_RETURN_KIND: AsmReg = AsmReg::Cx;

pub const JIT_CALL_LIVE_INS: &[AsmReg] = &[JIT_ARGS, JIT_VM, JIT_RT, JIT_ENTRYPOINTS];

/// Register used to hold the (hidden) pointer to JitResult upon entering the JIT land.
pub const ENTRY_RESULT_OUT_ARG: AsmReg = SYSV_GP_ARG_REGS[0];

/// Register used to hold the *mut Vm ptr upon entering the JIT land.
pub const ENTRY_VM_ARG: AsmReg = SYSV_GP_ARG_REGS[1];

/// Register used to hold the *const Rt ptr upon entering the JIT land.
pub const ENTRY_RT_ARG: AsmReg = SYSV_GP_ARG_REGS[2];

/// Register used to hold the *const Value ptr (args) upon entering the JIT land.
pub const ENTRY_ARGS_ARG: AsmReg = SYSV_GP_ARG_REGS[3];

/// Register used to hold the *const u8 ptr (body) upon entering the JIT land.
pub const ENTRY_BODY_ARG: AsmReg = SYSV_GP_ARG_REGS[4];

/// Register used to hold the `end_pc` upon entering the JIT land.
pub const ENTRY_END_PC_ARG: AsmReg = SYSV_GP_ARG_REGS[5];

/// Stack offset used for `native_entrypoints` upon entering the JIT land.
pub const ENTRY_ENTRYPOINTS_RBP_OFFSET: isize = 16;

/// Registers containing inputs to a JIT-to-JIT call.
pub const JIT_ENTRY_PRESERVED: &[AsmReg] = &[JIT_RESULT_STORAGE, JIT_VM, JIT_RT, JIT_ENTRYPOINTS];

// Register used to load an indirect runtime-call target.
// Must not overlap with the runtime function's argument registers.
pub const RT_CALL_SCRATCH: AsmReg = GP_SCRATCH_1;
