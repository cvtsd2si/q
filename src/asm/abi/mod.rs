pub mod amd64;
pub mod internal;

use crate::asm::{
    abi::{
        amd64::{SYSV_GP_ARG_REGS, SYSV_SSE_ARG_REGS},
        internal::JIT_CALL_LIVE_INS,
    },
    r#gen::{AsmReg, CallConv},
};

pub fn call_live_in_regs(conv: CallConv) -> impl Iterator<Item = AsmReg> {
    let (gp, sse) = match conv {
        CallConv::SystemV => (SYSV_GP_ARG_REGS, SYSV_SSE_ARG_REGS),
        CallConv::Jit => (JIT_CALL_LIVE_INS, &[][..]),
    };

    gp.iter().chain(sse.iter()).copied()
}
