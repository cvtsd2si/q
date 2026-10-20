use crate::{
    asm::{
        emitter::Emit,
        fixup::Fixup,
        r#gen::{AsmFunc, AsmInstr, AsmInstrBinKind, AsmOp, insert_epilogue, insert_prologue},
        regalloc::Regalloc,
    },
    ir::r#gen::{Codegen, CodegenContext, IRFunc, IRInstr, IRProg},
    runtime::{value::Value, vm::Vm},
};
use libc::{
    MAP_ANON, MAP_FAILED, MAP_PRIVATE, PROT_EXEC, PROT_READ, PROT_WRITE, mmap, mprotect, munmap,
};
use rsasm::{
    arch,
    assembler::{Assembler, Options},
    section::SectionId,
};
use std::fmt::Write;

pub struct JitCompiler {}

impl JitCompiler {
    pub fn new() -> Self {
        Self {}
    }

    pub fn compile(
        ir: &IRProg,
        func: usize,
        /* What code to compile?  */
        start_ir: usize,
        end_ir: usize,
        /* What interpreted bytecode this native code replaces?  */
        start_pc: usize,
        end_pc: usize,
    ) -> Option<JitBlock> {
        if start_pc != 0 {
            return None;
        }

        let ir_func = ir.funcs.get(func)?;
        let region = &ir_func.body[start_ir..end_ir];

        if !ir_func.params.is_empty() {
            return None;
        }

        /* Reject anything that does not end up in language-level `return`.  */
        if !matches!(region.last(), Some(IRInstr::Ret { .. })) {
            return None;
        }

        /* Reject `IRInstr::Print` because the asm backend currently has no impl for this.  */
        if region
            .iter()
            .any(|instr| matches!(instr, IRInstr::Print { .. } | IRInstr::Call { .. }))
        {
            return None;
        }

        let region_func = IRFunc {
            name: format!("jit_{}_{}", ir_func.name, start_ir),
            params: ir_func.params.clone(),
            body: ir_func.body[start_ir..end_ir].to_vec(),
            ret_ty: ir_func.ret_ty,
        };

        let mut asm_func: AsmFunc = region_func.codegen(&mut CodegenContext::new()).ok()?;
        let mut asm_func = asm_func.regalloc().fixup();

        let body_name = region_func.name.clone();
        let mut body_txt = String::new();

        if let Some(instr) = asm_func.body.iter_mut().nth(2) {
            match instr {
                AsmInstr::Bin {
                    kind: AsmInstrBinKind::Sub,
                    lhs,
                    ..
                } => {
                    *lhs = AsmOp::Imm(asm_func.space as i64);
                }
                _ => unreachable!(),
            }
        }

        if asm_func.emit(&mut body_txt).is_err() {
            return None;
        }

        /* Build the `jit_entry` wrapper around the code we're compiling.  */
        let mut asm_txt = String::new();

        writeln!(asm_txt, "\t.global jit_entry").ok();
        writeln!(asm_txt, "jit_entry:").ok();

        insert_prologue(&mut asm_txt);

        /* Preserve callee-saved registers we use in our ABI.  */
        writeln!(asm_txt, "\tpushq %r12").ok();
        writeln!(asm_txt, "\tpushq %r13").ok();

        /* Since `JitResult` is a large struct, it is passed by a hidden mem ptr.
         *
         * %rdi = hidden *mut JitResult ptr
         * %rsi = *mut Vm  */
        writeln!(asm_txt, "\tmovq %rdi, %r12").ok();
        writeln!(asm_txt, "\tmovq %rsi, %r13").ok();

        /* Execute the compiled region.  */
        writeln!(asm_txt, "\tcall {}", body_name).ok();

        /* #[repr(C)]
         * pub struct JitResult {
         *     pub kind: u64,     // offset 0
         *     pub next_pc: u64,  // offset 8
         *     pub value: Value,  // offset 16
         * }
         *
         * ...and `Value` is:
         *      offset 16: tag
         *      offset 24: payload
         */
        writeln!(asm_txt, "\tmovq ${}, 0(%r12)", 1).ok(); // kind = 1 is JIT_RET
        writeln!(asm_txt, "\tmovq ${}, 8(%r12)", end_pc).ok();
        writeln!(asm_txt, "\tmovq %rdx, 16(%r12)").ok(); // value tag
        writeln!(asm_txt, "\tmovq %rax, 24(%r12)").ok(); // payload 
        writeln!(asm_txt, "\tmovq %r12, %rax").ok();

        /* Restore the native stack/register state and return to VM.  */
        writeln!(asm_txt, "\tpopq %r13").ok();
        writeln!(asm_txt, "\tpopq %r12").ok();

        insert_epilogue(&mut asm_txt);

        writeln!(asm_txt, "\tret").ok();

        writeln!(asm_txt).ok();

        asm_txt.push_str(&body_txt);

        eprintln!("[JIT ASM]\n{}", asm_txt);

        let arch = arch::lookup("x86-64").unwrap();
        let mut assembler = Assembler::new(arch, Options::new().with_relocatable(false));

        assembler.assemble_str("", &asm_txt);

        assert!(assembler.finish());

        let raw_bytes = assembler.section_bytes(SectionId(0));

        unsafe {
            let len = raw_bytes.len();

            let ptr = mmap(
                std::ptr::null_mut(),
                len,
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANON,
                -1,
                0,
            );

            if ptr == MAP_FAILED {
                panic!("mmap failed");
            }

            std::ptr::copy_nonoverlapping(raw_bytes.as_ptr(), ptr.cast::<u8>(), len);

            if mprotect(ptr, len, PROT_READ | PROT_EXEC) != 0 {
                munmap(ptr, len);
                panic!("mprotect failed");
            }

            let mem = ptr.cast::<u8>();

            let entry = std::mem::transmute::<*mut u8, JitEntry>(mem);

            Some(JitBlock {
                entry,
                start_pc,
                end_pc,
                mem,
                len,
            })
        }
    }
}

pub type JitEntry = unsafe extern "C" fn(*mut Vm) -> JitResult;

pub const JIT_THRESHOLD: u32 = 1;

pub struct JitBlock {
    pub entry: JitEntry,
    pub start_pc: usize,
    pub end_pc: usize,
    pub mem: *mut u8,
    pub len: usize,
}

impl Drop for JitBlock {
    fn drop(&mut self) {
        unsafe {
            munmap(self.mem.cast(), self.len);
        }
    }
}

/// ABI-facing result
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct JitResult {
    /* Two `kind`s so far:
     *  - 0: "I have finished executing the native code,
     *        but the vm code is still running.
     *  - 1: "I have finished executing the native code,
     *        and encountered the language-level `return`.  */
    pub kind: u64,

    /* `next_pc` tells `VM::exec()` where to continue after
     * getting back from executing the native code.  */
    pub next_pc: u64,

    /* The `Value` brought from the other side.  */
    pub value: Value,
}

/// VM-facing result
#[derive(Debug, Clone, Copy)]
pub enum JitExit {
    Continue(usize), /* Carries next_pc.  */
    Ret(Value),
}
