use crate::asm::r#gen::{AsmReg, AsmType, CallConv, RtFn};

/// SystemV AMD64 ABI (Linux x86-64)
///
///
/// Section 1: argument passing
///
/// According to the SystemV AMD64 ABI, the first six integer and ptr args
/// are passed to the called function in %rdi, %rsi, %rdx, %rcx, %r8, %r9.
///
/// Floating point args use the SIMD registers (%xmm0 through %xmm7).
///
/// Args that cannot be assigned the registers required by their ABI classification
/// are passed on the stack, where %rsp points to the return address of the caller,
/// and the first arg is at 8(%rsp).
///
/// It is *NOT* the case that every arg smaller than 8 bytes goes into a reg,
/// and everything larger goes on the stack.  Rather, every arg is classified
/// by the arg type and the layout (INTEGER, SSE, MEMORY, etc.)
///
/// Simple scalars (like ints and ptrs) are passed through general-purpose
/// registers, while floating point args are passed through SSE registers.
///
/// Structures are divided into eightbytes and each eightbyte is classified
/// according to its arg type.
///
///
/// Section 2: return values
///
/// Return value is passed through %rax (or %eax) for the INTEGER class,
/// and in %xmm0 for the SSE class.  
///
/// Aggregates may use a combination of both general-purpose and SSE registers.
///
/// If the return value cannot fit in a single register, additional registers
/// are used (%rdx and %xmm1), respectively.
///
/// If the return value is classifed as MEMORY, the caller must allocate space
/// for the returned struct and pass it as a ptr in %rdi.
///
/// Before returning, it places the result in %rax.
///
///
/// Section 3: register preservation
///
/// The SystemV AMD64 ABI also specifies which registers will survive the call
/// and not get clobbered by the called function.  There are two cases:
///
///  - callee saved registers:    these registers cannot be freely modified by the called
///    (%rbx, %rbp, %r12-%r15)    function, and they must contain the same value when the
///                               called function returns, as they had when it entered
///
///  - caller saved registers:    these registers can be freely clobbered by the called
///    (%rax, %rcx, %rdx, %rdi,   function, and the caller must preserve them if it wants
///     %rsi, %r8, %r9, %r10,     to use them past the call.
///     %r11, %xmm0-%xmm15, %rsp)
///
///
/// Section 4: stack alignment
///
/// Immediately before executing the `call` instr, the stack must be aligned to a 16-byte
/// boundary, that is %rsp must be divisible by 16.
///
/// The `call` instr conceptually does:
///      %rsp -= 8
///      %mem[%rsp] = ret_addr
///      %rip = addr_of(f)
/// When `f` executes `ret`, the processor pops the ret_addr from the stack and jumps to it.
///
///
/// Section 5: variadic functions
///
/// Variadic functions calls (e.g. `printf("%d", 42)`) follow the usual convention which is
/// the ptr to `"%d"` is passed in %rdi, and %esi needs to contain `42`.  However, they also
/// introduce a new rule: For calls that use a variable arg list, the caller must also set %al
/// to contain an upper bound (0-8) on the number of SSE regs used for passing args.

pub const SYSV_STACK_ALIGNMENT: usize = 16;

/// The amoutn of stack space in bytes occupied by a 64-bit pushq instruction.
pub const GP_PUSH_BYTES: usize = 8;

/// Save the low 64 bits of caller-saved XMM registers.
///
/// The allocator currently supports scalar f32/f64 values,
/// so an 8-byte movsd save preserves the live scalar value.
pub const SSE_CALL_SAVE_TY: AsmType = AsmType::Double;

/// General-purpose registers allocatable by the register allocator.
pub const GP_ALLOCATABLE: &[AsmReg] = &[AsmReg::R8, AsmReg::R9];

/// SSE registers allocatable by the register allocator.
pub const SSE_ALLOCATABLE: &[AsmReg] = &[
    AsmReg::Xmm0,
    AsmReg::Xmm1,
    AsmReg::Xmm2,
    AsmReg::Xmm3,
    AsmReg::Xmm4,
    AsmReg::Xmm5,
    AsmReg::Xmm6,
    AsmReg::Xmm7,
    /* AsmReg::Xmm8,
     * AsmReg::Xmm9,
     * AsmReg::Xmm10,
     * AsmReg::Xmm11,
     * AsmReg::Xmm12,
     * AsmReg::Xmm13,
     * AsmReg::Xmm14,
     * AsmReg::Xmm15,
     */
];

/// Caller-saved registers.
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
    /* AsmReg::Xmm8,
     * AsmReg::Xmm9,
     * AsmReg::Xmm10,
     * AsmReg::Xmm11,
     * AsmReg::Xmm12,
     * AsmReg::Xmm13,  */
    AsmReg::Xmm14,
    AsmReg::Xmm15,
];

/// General-purpose registers used for passing arguments to the called
/// function by the SystemV AMD64 ABI.
pub const SYSV_GP_ARG_REGS: &[AsmReg] = &[
    AsmReg::Di,
    AsmReg::Si,
    AsmReg::Dx,
    AsmReg::Cx,
    AsmReg::R8,
    AsmReg::R9,
];

/// SSE registers used for passing arguments to the called
/// function by the SystemV AMD64 ABI.
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

/// General-purpose scratch register #1.
pub const GP_SCRATCH_1: AsmReg = AsmReg::R10;
/// General-purpose scratch register #2.
pub const GP_SCRATCH_2: AsmReg = AsmReg::R11;

/// SSE scratch register #1.
pub const SSE_SCRATCH_1: AsmReg = AsmReg::Xmm14;
/// SSE scratch register #2.
pub const SSE_SCRATCH_2: AsmReg = AsmReg::Xmm15;

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

/// Register used by the SystemV AMD64 ABI to hold the return value.
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
