//! SystemV AMD64 ABI (Linux x86-64)
//!
//!
//! Section 1: argument passing
//!
//! According to the SystemV AMD64 ABI, the first six integer and ptr args
//! are passed to the called function in %rdi, %rsi, %rdx, %rcx, %r8, %r9.
//!
//! Floating point args use the SIMD registers (%xmm0 through %xmm7).
//!
//! Args that cannot be assigned the registers required by their ABI classification
//! are passed on the stack, where %rsp points to the return address of the caller,
//! and the first arg is at 8(%rsp).
//!
//! It is *NOT* the case that every arg smaller than 8 bytes goes into a reg,
//! and everything larger goes on the stack.  Rather, every arg is classified
//! by the arg type and the layout (INTEGER, SSE, MEMORY, etc.)
//!
//! Simple scalars (like ints and ptrs) are passed through general-purpose
//! registers, while floating point args are passed through SSE registers.
//!
//! Structures are divided into eightbytes and each eightbyte is classified
//! according to its arg type.
//!
//!
//! Section 2: return values
//!
//! Return value is passed through %rax (or %eax) for the INTEGER class,
//! and in %xmm0 for the SSE class.  
//!
//! Aggregates may use a combination of both general-purpose and SSE registers.
//!
//! If the return value cannot fit in a single register, additional registers
//! are used (%rdx and %xmm1), respectively.
//!
//! If the return value is classifed as MEMORY, the caller must allocate space
//! for the returned struct and pass it as a ptr in %rdi.
//!
//! Before returning, it places the result in %rax.
//!
//!
//! Section 3: register preservation
//!
//! The SystemV AMD64 ABI also specifies which registers will survive the call
//! and not get clobbered by the called function.  There are two cases:
//!
//!  - callee saved registers:      these registers cannot be freely modified by the called
//!    (%rbx, %rbp, %r12, %r13,     function, and they must contain the same value when the
//!    %r14, %r15)                  called function returns, as they had when it entered
//!
//!  - caller saved registers:      these registers can be freely clobbered by the called
//!    (%rax, %rcx, %rdx, %rdi,     function, and the caller must preserve them if it wants
//!    %rsi, %r8, %r9, %r10,        to use them past the call.
//!    %r11, %xmm0-%xmm15, %rsp)
//!
//!
//! Section 4: stack alignment
//!
//! Immediately before executing the `call` instr, the stack must be aligned to a 16-byte
//! boundary, that is %rsp must be divisible by 16.
//!
//! The `call` instr conceptually does:
//!      %rsp -= 8
//!      %mem[%rsp] = ret_addr
//!      %rip = addr_of(f)
//! When `f` executes `ret`, the processor pops the ret_addr from the stack and jumps to it.
//!
//!
//! Section 5: variadic functions
//!
//! Variadic functions calls (e.g. `printf("%d", 42)`) follow the usual convention which is
//! the ptr to `"%d"` is passed in %rdi, and %esi needs to contain `42`.  However, they also
//! introduce a new rule: For calls that use a variable arg list, the caller must also set %al
//! to contain an upper bound (0-8) on the number of SSE regs used for passing args.

use crate::codegen::asm::r#gen::{AsmReg, AsmType, RtFn};

pub const SYSV_STACK_ALIGNMENT: usize = 16;

/// The amoutn of stack space in bytes occupied by a 64-bit pushq instruction.
pub const GP_PUSH_BYTES: usize = 8;

/// Register used by the SystemV AMD64 ABI to hold the return value.
pub const SYSV_GP_RETURN: AsmReg = AsmReg::Ax;

/// Save the low 64 bits of caller-saved XMM registers.
///
/// The allocator currently supports scalar f32/f64 values,
/// so an 8-byte movsd save preserves the live scalar value.
pub const SSE_CALL_SAVE_TY: AsmType = AsmType::Double;

/// General-purpose registers allocatable by the register allocator.
pub const GP_ALLOCATABLE: &[AsmReg] = &[AsmReg::R8, AsmReg::R9]; // FIXME: add more GP regs

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
    AsmReg::Xmm8,
    AsmReg::Xmm9,
    AsmReg::Xmm10,
    AsmReg::Xmm11,
    AsmReg::Xmm12,
    AsmReg::Xmm13,
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
    AsmReg::Xmm8,
    AsmReg::Xmm9,
    AsmReg::Xmm10,
    AsmReg::Xmm11,
    AsmReg::Xmm12,
    AsmReg::Xmm13,
    AsmReg::Xmm14,
    AsmReg::Xmm15,
];

/// Callee-saved registers.
pub const CALLEE_SAVED: &[AsmReg] = &[
    AsmReg::Bx,
    AsmReg::Bp,
    AsmReg::R12,
    AsmReg::R13,
    AsmReg::R14,
    AsmReg::R15,
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

pub fn is_caller_saved(reg: AsmReg) -> bool {
    CALLER_SAVED.contains(&reg)
}

pub fn is_callee_saved(reg: AsmReg) -> bool {
    CALLEE_SAVED.contains(&reg)
}

pub fn is_allocatable(reg: AsmReg) -> bool {
    GP_ALLOCATABLE.contains(&reg) || SSE_ALLOCATABLE.contains(&reg)
}

pub fn needs_caller_save(reg: AsmReg) -> bool {
    is_allocatable(reg) && is_caller_saved(reg)
}

pub fn needs_callee_save(reg: AsmReg) -> bool {
    is_allocatable(reg) && is_callee_saved(reg)
}

pub fn rt_arg_regs(target: RtFn) -> impl Iterator<Item = AsmReg> {
    let count = match target {
        RtFn::Print => 3,
        RtFn::Call => 5,
        RtFn::Strcmp => 2,
    };

    SYSV_GP_ARG_REGS.iter().copied().take(count)
}

pub fn rt_arg_reg(target: RtFn, index: usize) -> AsmReg {
    rt_arg_regs(target).nth(index).unwrap()
}
