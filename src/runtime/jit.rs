use crate::{
    asm::{
        abi::{
            ENTRY_ARGS_ARG, ENTRY_BODY_ARG, ENTRY_END_PC_ARG, ENTRY_ENTRYPOINTS_RBP_OFFSET,
            ENTRY_RESULT_OUT_ARG, ENTRY_RT_ARG, ENTRY_VM_ARG, GP_SCRATCH_1, JIT_ARGS,
            JIT_ENTRY_PRESERVED, JIT_ENTRYPOINTS, JIT_RESULT_STORAGE, JIT_RETURN_KIND,
            JIT_RETURN_PAYLOAD, JIT_RETURN_TAG, JIT_RT, JIT_VM, SYSV_GP_RETURN,
            SYSV_STACK_ALIGNMENT,
        },
        emitter::Emit,
        fixup::Fixup,
        r#gen::{
            AsmFunc, AsmInstr, AsmInstrBinKind, AsmOp, AsmReg, AsmStaticConst, AsmType, CallConv,
            insert_epilogue, insert_prologue,
        },
        regalloc::Regalloc,
    },
    bytecode::r#gen::BytecodeFunc,
    ir::r#gen::{Codegen, CodegenContext, Ctx, IRFunc, IRInstr, IRProg},
    runtime::{
        rt::Rt,
        value::{Value, ValueTag},
        vm::Vm,
    },
};
use keystone_engine::{Arch, Keystone, Mode, OptionType, OptionValue};
use libc::{
    MAP_ANON, MAP_FAILED, MAP_PRIVATE, PROT_EXEC, PROT_READ, PROT_WRITE, mmap, mprotect, munmap,
};

pub struct JitCompiler {
    assembler: Keystone,
}

impl JitCompiler {
    pub fn new() -> Self {
        let assembler = Keystone::new(Arch::X86, Mode::MODE_64).unwrap();
        assembler
            .option(OptionType::SYNTAX, OptionValue::SYNTAX_ATT)
            .unwrap();

        Self { assembler }
    }

    pub fn compile(
        &mut self,
        ir: &IRProg,
        func: usize,
        /* What code to compile?  */
        _start_ir: usize,
        end_ir: usize,
        /* What interpreted bytecode this native code replaces?  */
        start_pc: usize,
        end_pc: usize,
    ) -> Result<Option<JitBlock>, JitError> {
        if start_pc != 0 {
            return Ok(None);
        }

        let ir_func = ir.funcs.get(func).unwrap();

        /* FIXME: currently compiling entire funcs  */
        let start_ir = 0;

        let region = &ir_func.body[start_ir..end_ir];

        /* Reject anything that does not end up in language-level `return`.  */
        if !matches!(region.last(), Some(IRInstr::Ret { .. })) {
            return Ok(None);
        }

        let region_func = IRFunc {
            name: format!("jit_{}_{}", ir_func.name, start_ir),
            params: ir_func.params.clone(),
            body: ir_func.body[start_ir..end_ir].to_vec(),
            ret_ty: ir_func.ret_ty,
        };

        let mut asm_txt = String::new();

        let mut ctx = CodegenContext::new();
        for func in ir.funcs.iter() {
            ctx.register::<BytecodeFunc>(&func.name);
        }

        /* Compiled fn emission.  */
        let mut asm_func: AsmFunc = region_func.codegen(&mut ctx).map_err(|_| JitError {
            msg: "jit codegen failed".into(),
        })?;

        /* Append the runtime error handler before register allocation.  */
        asm_func
            .body
            .push(AsmInstr::Label(format!("{}_rt_err", region_func.name)));

        asm_func.body.extend(self.mk_rt_err());

        let mut asm_func = asm_func
            .regalloc()
            .map_err(|e| JitError { msg: e.msg })?
            .fixup()
            .map_err(|e| JitError { msg: e.msg })?;

        /* Backpatch the amount of stack space required.  */
        let frame_space = asm_func.space as i64;

        let frame_allocation = asm_func.body.iter_mut().find(|instr| {
            matches!(
                instr,
                AsmInstr::Bin {
                    kind: AsmInstrBinKind::Sub,
                    lhs: AsmOp::Imm(0),
                    rhs: AsmOp::Reg(AsmReg::Sp),
                    asm_ty: AsmType::Quad,
                }
            )
        });

        match frame_allocation {
            Some(AsmInstr::Bin { lhs, .. }) => {
                *lhs = AsmOp::Imm(frame_space);
            }

            _ => {
                return Err(JitError {
                    msg: "missing stack-frame allocation in generated prologue".into(),
                });
            }
        }

        asm_func.emit(&mut asm_txt).map_err(|_e| JitError {
            msg: "jit code emission failed".into(),
        })?;

        for konst in &ctx.static_consts {
            AsmStaticConst {
                name: konst.name.clone(),
                init: konst.init,
                ty: konst.ty,
                alignment: konst.alignment,
            }
            .emit(&mut asm_txt)
            .map_err(|_| JitError {
                msg: "JIT constant emission failed".into(),
            })?;
        }

        let code = self.asm_exe(&asm_txt)?;

        Ok(Some(JitBlock {
            start_pc,
            end_pc,
            code,
        }))
    }

    fn asm_exe(&mut self, asm_txt: &str) -> Result<ExecutableMem, JitError> {
        if cfg!(debug_assertions) {
            eprintln!("{}", asm_txt);
        }

        let r = self
            .assembler
            .asm(asm_txt.to_string(), 0)
            .map_err(|e| JitError {
                msg: format!("JIT asm failed: {e}"),
            })?;

        ExecutableMem::new(&r.bytes)
    }

    pub fn compile_entry(&mut self) -> Result<JitTrampoline, JitError> {
        let f = AsmFunc {
            name: "jit_shared_entry".into(),
            body: self.mk_entry(),
            space: 0,
        };

        let mut asm_txt = String::new();

        f.emit(&mut asm_txt).map_err(|_| JitError {
            msg: "jit entry emission failed".into(),
        })?;

        let code = self.asm_exe(&asm_txt)?;

        let entry = unsafe { std::mem::transmute::<*const u8, JitEntry>(code.as_ptr()) };

        Ok(JitTrampoline { entry, _code: code })
    }

    fn mk_entry(&mut self) -> Vec<AsmInstr> {
        let mut body = vec![];

        insert_prologue(&mut body);

        let stack_slot_bytes = AsmType::Quad.size();
        let saved_bytes = (JIT_ENTRY_PRESERVED.len() + 1) * stack_slot_bytes;
        let call_padding =
            (SYSV_STACK_ALIGNMENT - saved_bytes % SYSV_STACK_ALIGNMENT) % SYSV_STACK_ALIGNMENT;

        body.push(AsmInstr::Comment(
            "As a callee (from Rust Vm's perspective), preserve callee-saved".into(),
        ));
        body.push(AsmInstr::Comment(
            "registers used in the internal ABI.".into(),
        ));
        body.extend(
            JIT_ENTRY_PRESERVED
                .iter()
                .copied()
                .map(|reg| AsmInstr::Push(AsmOp::Reg(reg))),
        );

        body.extend([
            AsmInstr::Newline,
            AsmInstr::Comment(
                "As a caller (from the perspective of the natively compiled function)".into(),
            ),
            AsmInstr::Comment(
                "preserve caller-saved registers which must survive the call to the".into(),
            ),
            AsmInstr::Comment("aforementioned natively compiled function.".into()),
            AsmInstr::Newline,
            AsmInstr::Comment("Save the entry's `end_pc` across the native call.".into()),
            AsmInstr::Push(AsmOp::Reg(ENTRY_END_PC_ARG)),
        ]);

        body.push(AsmInstr::Newline);

        body.extend([
            AsmInstr::Comment("Save the hidden JitResult return pointer.".into()),
            AsmInstr::Mov {
                src: AsmOp::Reg(ENTRY_RESULT_OUT_ARG),
                dst: AsmOp::Reg(JIT_RESULT_STORAGE),
                asm_ty: AsmType::Quad,
            },
            AsmInstr::Newline,
            AsmInstr::Comment("Initialize the reserved VM register.".into()),
            AsmInstr::Mov {
                src: AsmOp::Reg(ENTRY_VM_ARG),
                dst: AsmOp::Reg(JIT_VM),
                asm_ty: AsmType::Quad,
            },
            AsmInstr::Newline,
            AsmInstr::Comment("Initialize the reserved runtime table register.".into()),
            AsmInstr::Mov {
                src: AsmOp::Reg(ENTRY_RT_ARG),
                dst: AsmOp::Reg(JIT_RT),
                asm_ty: AsmType::Quad,
            },
            AsmInstr::Newline,
            AsmInstr::Comment("Forward the argument-array pointer to the JIT ABI.".into()),
            AsmInstr::Mov {
                src: AsmOp::Reg(ENTRY_ARGS_ARG),
                dst: AsmOp::Reg(JIT_ARGS),
                asm_ty: AsmType::Quad,
            },
            AsmInstr::Newline,
            AsmInstr::Comment("Load the entrypoint table pointer from the stack argument.".into()),
            AsmInstr::Mov {
                src: AsmOp::Mem(AsmReg::Bp, ENTRY_ENTRYPOINTS_RBP_OFFSET),
                dst: AsmOp::Reg(JIT_ENTRYPOINTS),
                asm_ty: AsmType::Quad,
            },
        ]);

        if call_padding != 0 {
            body.push(AsmInstr::Comment(
                "Align the stack for the native call.".into(),
            ));
            body.push(AsmInstr::Bin {
                kind: AsmInstrBinKind::Sub,
                lhs: AsmOp::Imm(call_padding as i64),
                rhs: AsmOp::Reg(AsmReg::Sp),
                asm_ty: AsmType::Quad,
            });
        }

        body.push(AsmInstr::Newline);

        body.extend([
            AsmInstr::Comment("Call the compiled function address.".into()),
            AsmInstr::CallIndirect {
                target: ENTRY_BODY_ARG,
                conv: CallConv::Jit,
            },
        ]);

        body.push(AsmInstr::Newline);

        body.extend([
            AsmInstr::Comment("According to the internal ABI, after the call instr".into()),
            AsmInstr::Comment(
                "JIT return registers now contain the exit kind, tag and payload.".into(),
            ),
            AsmInstr::Comment("The trampoline still holds the JitResult output pointer.".into()),
        ]);

        body.push(AsmInstr::Newline);

        body.extend([
            AsmInstr::Comment("Store the JIT exit kind.".into()),
            AsmInstr::Mov {
                src: AsmOp::Reg(JIT_RETURN_KIND),
                dst: AsmOp::Mem(
                    JIT_RESULT_STORAGE,
                    std::mem::offset_of!(JitResult, kind) as isize,
                ),
                asm_ty: AsmType::Quad,
            },
            AsmInstr::Newline,
            AsmInstr::Comment("Read the saved next_pc from the stack.".into()),
            AsmInstr::Mov {
                src: AsmOp::Mem(AsmReg::Sp, call_padding as isize),
                dst: AsmOp::Reg(GP_SCRATCH_1),
                asm_ty: AsmType::Quad,
            },
            AsmInstr::Newline,
            AsmInstr::Comment("Store next_pc in the JitResult.".into()),
            AsmInstr::Mov {
                src: AsmOp::Reg(GP_SCRATCH_1),
                dst: AsmOp::Mem(
                    JIT_RESULT_STORAGE,
                    std::mem::offset_of!(JitResult, next_pc) as isize,
                ),
                asm_ty: AsmType::Quad,
            },
            AsmInstr::Newline,
            AsmInstr::Comment("Store the return value tag.".into()),
            AsmInstr::Mov {
                src: AsmOp::Reg(JIT_RETURN_TAG),
                dst: AsmOp::Mem(
                    JIT_RESULT_STORAGE,
                    (std::mem::offset_of!(JitResult, value) + std::mem::offset_of!(Value, tag))
                        as isize,
                ),
                asm_ty: AsmType::Quad,
            },
            AsmInstr::Newline,
            AsmInstr::Comment("Store the return payload or runtime error.".into()),
            AsmInstr::Mov {
                src: AsmOp::Reg(JIT_RETURN_PAYLOAD),
                dst: AsmOp::Mem(
                    JIT_RESULT_STORAGE,
                    (std::mem::offset_of!(JitResult, value) + std::mem::offset_of!(Value, payload))
                        as isize,
                ),
                asm_ty: AsmType::Quad,
            },
            AsmInstr::Newline,
            AsmInstr::Comment(
                "Return the JitResult ptr according to the SystemV AMD64 ABI.".into(),
            ),
            AsmInstr::Mov {
                src: AsmOp::Reg(JIT_RESULT_STORAGE),
                dst: AsmOp::Reg(SYSV_GP_RETURN),
                asm_ty: AsmType::Quad,
            },
        ]);

        body.push(AsmInstr::Newline);

        if call_padding != 0 {
            body.push(AsmInstr::Bin {
                kind: AsmInstrBinKind::Add,
                lhs: AsmOp::Imm(call_padding as i64),
                rhs: AsmOp::Reg(AsmReg::Sp),
                asm_ty: AsmType::Quad,
            });
        }

        body.extend([
            AsmInstr::Comment("Restore registers and return to VM.".into()),
            AsmInstr::Pop(AsmOp::Reg(ENTRY_END_PC_ARG)),
        ]);

        body.extend(
            JIT_ENTRY_PRESERVED
                .iter()
                .rev()
                .copied()
                .map(|reg| AsmInstr::Pop(AsmOp::Reg(reg))),
        );

        body.push(AsmInstr::Newline);

        insert_epilogue(&mut body);
        body.push(AsmInstr::Ret);

        body
    }

    fn mk_rt_err(&mut self) -> Vec<AsmInstr> {
        /* Build the shared runtime error handler.
         *
         * Upon entry:
         *      %rax = rt err code (zero-extended u32)
         *
         * Upon exit:
         *      %rcx = JIT_ERROR
         *      %rax = rt err code
         *      %rdx = ValueTag::Void  */
        let mut body = vec![
            AsmInstr::Comment("Shared rt err handler.".into()),
            AsmInstr::Comment("Set the JIT exit kind to JIT_ERROR.".into()),
            AsmInstr::Mov {
                src: AsmOp::Imm(JIT_ERROR as i64),
                dst: AsmOp::Reg(JIT_RETURN_KIND),
                asm_ty: AsmType::Quad,
            },
            AsmInstr::Newline,
            AsmInstr::Comment("Set the return tag to void".into()),
            AsmInstr::Mov {
                src: AsmOp::Imm(ValueTag::Void as i64),
                dst: AsmOp::Reg(JIT_RETURN_TAG),
                asm_ty: AsmType::Quad,
            },
            AsmInstr::Newline,
        ];

        insert_epilogue(&mut body);
        body.push(AsmInstr::Ret);

        body
    }
}

impl Default for JitCompiler {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ExecutableMem {
    pub mem: *mut u8,
    pub len: usize,
}

impl ExecutableMem {
    pub fn new(bytes: &[u8]) -> Result<Self, JitError> {
        if bytes.is_empty() {
            return Err(JitError {
                msg: "empty machine code".into(),
            });
        }

        unsafe {
            let len = bytes.len();

            let ptr = mmap(
                std::ptr::null_mut(),
                len,
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANON,
                -1,
                0,
            );

            if ptr == MAP_FAILED {
                return Err(JitError {
                    msg: format!("mmap failed: {}", std::io::Error::last_os_error()),
                });
            }

            std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.cast::<u8>(), len);

            if mprotect(ptr, len, PROT_READ | PROT_EXEC) != 0 {
                let err = std::io::Error::last_os_error();
                munmap(ptr, len);
                return Err(JitError {
                    msg: format!("mprotect failed: {}", err),
                });
            }

            Ok(Self {
                mem: ptr.cast::<u8>(),
                len,
            })
        }
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.mem.cast_const()
    }
}

impl Drop for ExecutableMem {
    fn drop(&mut self) {
        unsafe {
            munmap(self.mem.cast(), self.len);
        }
    }
}

pub type JitEntry = unsafe extern "C" fn(
    *mut Vm,
    *const Rt,
    *const Value, /* parameters  */
    *const u8,    /* body  */
    usize,        /* next_pc  */
    *const usize, /* native entry point table  */
) -> JitResult;

pub const JIT_THRESHOLD: u32 = 1;

pub struct JitBlock {
    pub start_pc: usize,
    pub end_pc: usize,
    pub code: ExecutableMem,
}

pub struct JitTrampoline {
    pub entry: JitEntry,
    _code: ExecutableMem,
}

pub const JIT_CONTINUE: u64 = 0;
pub const JIT_RET: u64 = 1;
pub const JIT_ERROR: u64 = 2;

/// ABI-facing result
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct JitResult {
    pub kind: u64,
    pub next_pc: u64,
    pub value: Value,
}

/// VM-facing result
#[derive(Debug, Clone, Copy)]
pub enum JitExit {
    Continue(usize), /* Carries next_pc.  */
    Ret(Value),
    Error(u32),
}

#[derive(Debug)]
pub struct JitError {
    pub msg: String,
}

impl std::error::Error for JitError {}

impl std::fmt::Display for JitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "jit: {}", self.msg)
    }
}
