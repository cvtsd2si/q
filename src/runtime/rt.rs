use crate::runtime::{
    value::{Value, ValueTag, print_value},
    vm::Vm,
};

#[repr(C)]
pub struct Rt {
    pub print: extern "C" fn(
        *mut Vm,
        u64, // ValueTag
        u64, // payload
    ) -> u32,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtResult {
    Ok = 0,
    Io = 1,
}

pub static RT: Rt = Rt { print: jit_print };

pub extern "C" fn jit_print(vm: *mut Vm, tag: u64, payload: u64) -> u32 {
    let tag = match ValueTag::try_from(tag) {
        Ok(tag) => tag,
        Err(_) => std::process::abort(),
    };

    let v = Value { tag, payload };

    match print_value(v) {
        Ok(()) => RtResult::Ok as u32,
        Err(_) => RtResult::Io as u32,
    }
}
