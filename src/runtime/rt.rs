use crate::runtime::{
    value::{Value, ValueTag, print_value},
    vm::{Vm, VmError},
};

#[repr(C)]
pub struct Rt {
    pub print: extern "C" fn(
        *mut Vm,
        u64, // ValueTag
        u64, // payload
    ) -> u32,

    pub call: extern "C" fn(
        *mut Vm,
        usize,      // fn idx
        *const u64, // payloads
        usize,      // argc
        *mut Value, // retval
    ) -> u32,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtResult {
    Ok = 0,
    Io = 1,
}

pub static RT: Rt = Rt {
    print: jit_print,
    call: jit_call,
};

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

pub extern "C" fn jit_call(
    vm: *mut Vm,
    target: usize,
    args: *const u64,
    argc: usize,
    out: *mut Value,
) -> u32 {
    unsafe {
        /* Ensure retval is properly inited.  */
        *out = Value {
            payload: 0,
            tag: ValueTag::Void,
        };

        let vm = &mut *vm;
        let args = std::slice::from_raw_parts(args, argc);

        match vm.invoke_from_jit(target, args) {
            Ok(v) => {
                *out = v;
                0
            }
            Err(VmError::Runtime(code)) => code,
        }
    }
}
