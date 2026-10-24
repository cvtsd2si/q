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

    pub call: unsafe extern "C" fn(
        *mut Vm,
        usize,        // fn idx
        *const Value, // args
        usize,        // argc
        *mut Value,   // retval
    ) -> u32,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtResult {
    Ok = 0,
    Io = 1,
    Internal = 2,
}

pub static RT: Rt = Rt {
    print: jit_print,
    call: jit_call,
};

pub extern "C" fn jit_print(_vm: *mut Vm, tag: u64, payload: u64) -> u32 {
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

/// Invokes a Vm function from native code.
///
/// # Safety
///
/// The caller must guarantee that:
///
/// - `vm` is non-null, properly aligned, and points to a valid,
///   initialized `Vm` that is exclusively accessible for the
///   duration of this call.
/// - `args` points to `argc` initialized `Value` values in a valid,
///   properly aligned allocation.  The memory must remain readable
///   and unchanged for the duration of the call.
/// - `out` is non-null, properly aligned, and writable for one `Value`.
/// - The memory referenced by `out` does not overlap with `vm`
///   or the argument slice.
/// - `target` identifies a valid function in the VM.
///
/// The native caller must also use the correct C ABI.
pub unsafe extern "C" fn jit_call(
    vm: *mut Vm,
    target: usize,
    args: *const Value,
    argc: usize,
    out: *mut Value,
) -> u32 {
    // SAFETY:
    // This function, which represents the runtime trampoline,
    // is called by native code emitted by JitCompiler.
    //
    // The generated call sequence passes a valid Vm ptr,
    // an arg buffer containing `argc` initialized u64 values,
    // and writable storage for the retval.
    //
    // These ptrs remain valid for the duration of the call,
    // and the generated code must uphold their alignment and
    // aliasing requirements.
    unsafe {
        /* The native caller reads the `out` variable before
         * checking the runtime status.  Init it so that the
         * error path doesn't read an uninited value.  */
        out.write(Value {
            payload: 0,
            tag: ValueTag::Void,
        });

        let vm = &mut *vm;
        let args = if argc == 0 {
            &[]
        } else {
            std::slice::from_raw_parts(args, argc)
        };

        match vm.invoke_from_jit(target, args) {
            Ok(v) => {
                out.write(v);

                0
            }
            Err(VmError { msg: _, code }) => code,
        }
    }
}
