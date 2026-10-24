use crate::{
    asm::{
        emitter::Emit,
        fixup::Fixup,
        r#gen::{
            AsmFunc, AsmInstr, AsmInstrBinKind, AsmOp, AsmReg, insert_epilogue, insert_prologue,
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
use libc::{
    MAP_ANON, MAP_FAILED, MAP_PRIVATE, PROT_EXEC, PROT_READ, PROT_WRITE, mmap, mprotect, munmap,
};
use rsasm::{
    arch,
    assembler::{Assembler, Options},
    section::SectionId,
};

pub struct JitCompiler {
    assembler: Assembler,
}

impl JitCompiler {
    pub fn new() -> Self {
        Self {
            assembler: Assembler::new(
                arch::lookup("x86-64").unwrap(),
                Options::new().with_relocatable(false),
            ),
        }
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
            msg: "jit codegen failed".to_string(),
        })?;
        let mut asm_func = asm_func.regalloc().fixup();

        /* Backpatch the amount of stack space required.  */
        if let Some(instr) = asm_func
            .body
            .iter_mut()
            .filter(|instr| !matches!(instr, AsmInstr::Comment(_) | AsmInstr::Newline))
            .nth(2)
        {
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

        asm_func.emit(&mut asm_txt).map_err(|_e| JitError {
            msg: "jit code emission failed".to_string(),
        })?;

        /* Runtime error handler emission.  */
        AsmFunc {
            name: format!("{}_rt_err", region_func.name),
            body: self.mk_rt_err(),
            space: 0,
        }
        .emit(&mut asm_txt)
        .map_err(|_| JitError {
            msg: "jit entry emission failed".to_string(),
        })?;

        let code = self.asm_exe(&asm_txt)?;

        Ok(Some(JitBlock {
            start_pc,
            end_pc,
            code,
        }))
    }

    fn asm_exe(&mut self, asm_txt: &str) -> Result<ExecutableMem, JitError> {
        self.assembler.assemble_str("", &asm_txt);

        if !self.assembler.finish() {
            eprintln!(
                "{}",
                self.assembler
                    .diags()
                    .render(self.assembler.source_map(), false)
            );

            return Err(JitError {
                msg: "JIT asm failed".to_string(),
            });
        }

        for f in self.assembler.source_map().files() {
            println!("{}", f.src);
        }

        let bytes = self.assembler.section_bytes(SectionId(0));

        ExecutableMem::new(&bytes)
    }

    pub fn compile_entry(&mut self) -> Result<JitTrampoline, JitError> {
        let entry = AsmFunc {
            name: "jit_shared_entry".to_string(),
            body: self.mk_entry(),
            space: 0,
        };

        let mut asm_txt = String::new();

        entry.emit(&mut asm_txt).map_err(|_| JitError {
            msg: "jit entry emission failed".to_string(),
        })?;

        let code = self.asm_exe(&asm_txt)?;

        let entry = unsafe { std::mem::transmute::<*const u8, JitEntry>(code.as_ptr()) };

        Ok(JitTrampoline { entry, _code: code })
    }

    fn mk_entry(&mut self) -> Vec<AsmInstr> {
        let mut body = vec![];

        insert_prologue(&mut body);

        /*
         * %rdi = hidden *mut JitResult ptr
         * %rsi = *mut Vm
         * %rdx = *const Rt
         * %rcx = *const Value (args)
         * %r8  = *const u8 (compiled fn addr)
         * %r9 = usize (end_pc)
         * 16(%rbp) = usize (ptr to `native_entrypoints`)
         */

        /* Internal ABI:
         * %r12 = *mut JitResult
         * %r13 = *mut Vm
         * %r14 = *const Rt
         * %rdx = *const Value (args)
         */
        body.push(AsmInstr::Comment(
            "As a callee (from Rust Vm's perspective), preserve callee-saved".to_string(),
        ));
        body.push(AsmInstr::Comment(
            "registers used in the internal ABI.".to_string(),
        ));
        body.extend([
            AsmInstr::Push(AsmOp::Reg(AsmReg::R12)),
            AsmInstr::Push(AsmOp::Reg(AsmReg::R13)),
            AsmInstr::Push(AsmOp::Reg(AsmReg::R14)),
            AsmInstr::Push(AsmOp::Reg(AsmReg::R15)),
        ]);

        body.extend([
            AsmInstr::Newline,
            AsmInstr::Comment(
                "As a caller (from the perspective of the natively compiled function)".to_string(),
            ),
            AsmInstr::Comment(
                "preserve caller-saved registers which must survive the call to the".to_string(),
            ),
            AsmInstr::Comment("aforementioned natively compiled function.".to_string()),
            AsmInstr::Newline,
            AsmInstr::Comment("%r9 contains `end_pc`.".to_string()),
            AsmInstr::Push(AsmOp::Reg(AsmReg::R9)),
        ]);

        body.push(AsmInstr::Newline);

        /* FIXME: maintain 16-byte stack alignment before calls.  */
        body.push(AsmInstr::Comment(
            "After the five pushes, the stack is misaligned by 8,".to_string(),
        ));
        body.push(AsmInstr::Comment(
            "so its 16-byte alignment needs restored.".to_string(),
        ));
        body.push(AsmInstr::Bin {
            kind: AsmInstrBinKind::Sub,
            lhs: AsmOp::Imm(8),
            rhs: AsmOp::Reg(AsmReg::Sp),
        });

        body.push(AsmInstr::Newline);

        body.extend([
            AsmInstr::Comment("%rdi = *mut JitResult".to_string()),
            AsmInstr::Mov {
                src: AsmOp::Reg(AsmReg::Di),
                dst: AsmOp::Reg(AsmReg::R12),
            },
            AsmInstr::Newline,
            AsmInstr::Comment("%rsi = *mut Vm".to_string()),
            AsmInstr::Mov {
                src: AsmOp::Reg(AsmReg::Si),
                dst: AsmOp::Reg(AsmReg::R13),
            },
            AsmInstr::Newline,
            AsmInstr::Comment("%rsi = *const Rt".to_string()),
            AsmInstr::Mov {
                src: AsmOp::Reg(AsmReg::Dx),
                dst: AsmOp::Reg(AsmReg::R14),
            },
            AsmInstr::Newline,
            AsmInstr::Comment("%rcx = *const Value (args)".to_string()),
            AsmInstr::Mov {
                src: AsmOp::Reg(AsmReg::Cx),
                dst: AsmOp::Reg(AsmReg::Di),
            },
            AsmInstr::Newline,
            AsmInstr::Comment("16(%rbp) = usize (ptr to `native_entrypoints`)".to_string()),
            AsmInstr::Mov {
                src: AsmOp::Stack(16),
                dst: AsmOp::Reg(AsmReg::R15),
            },
        ]);

        body.push(AsmInstr::Newline);

        body.extend([
            AsmInstr::Comment("%r8 = *const u8 (compiled fn addr)".to_string()),
            AsmInstr::CallIndirect { target: AsmReg::R8 },
        ]);

        body.push(AsmInstr::Newline);

        body.extend([
            AsmInstr::Comment("According to the internal ABI, after the call instr".to_string()),
            AsmInstr::Comment("%rax, %rcx, %rdx, will have contained the result,".to_string()),
            AsmInstr::Comment("while, remember, %r12 contains *mut JitResult.".to_string()),
        ]);

        body.push(AsmInstr::Newline);

        body.extend([
            AsmInstr::Comment("%rcx = JIT_CONTINUE | JIT_RET | JIT_ERROR".to_string()),
            AsmInstr::Mov {
                src: AsmOp::Reg(AsmReg::Cx),
                dst: AsmOp::Mem(AsmReg::R12, std::mem::offset_of!(JitResult, kind) as isize),
            },
            AsmInstr::Newline,
            AsmInstr::Comment("8(%rsp) = `next_pc`".to_string()),
            AsmInstr::Mov {
                src: AsmOp::Mem(AsmReg::Sp, 8),
                dst: AsmOp::Reg(AsmReg::R10),
            },
            AsmInstr::Newline,
            AsmInstr::Comment("%r10 = `next_pc`".to_string()),
            AsmInstr::Mov {
                src: AsmOp::Reg(AsmReg::R10),
                dst: AsmOp::Mem(
                    AsmReg::R12,
                    std::mem::offset_of!(JitResult, next_pc) as isize,
                ),
            },
            AsmInstr::Newline,
            AsmInstr::Comment("%rdx = ValueTag".to_string()),
            AsmInstr::Mov {
                src: AsmOp::Reg(AsmReg::Dx),
                dst: AsmOp::Mem(
                    AsmReg::R12,
                    (std::mem::offset_of!(JitResult, value) + std::mem::offset_of!(Value, tag))
                        as isize,
                ),
            },
            AsmInstr::Newline,
            AsmInstr::Comment("%rax = payload | err code".to_string()),
            AsmInstr::Mov {
                src: AsmOp::Reg(AsmReg::Ax),
                dst: AsmOp::Mem(
                    AsmReg::R12,
                    (std::mem::offset_of!(JitResult, value) + std::mem::offset_of!(Value, payload))
                        as isize,
                ),
            },
            AsmInstr::Newline,
            AsmInstr::Comment(
                "Return the JitResult ptr according to the SystemV AMD64 ABI.".to_string(),
            ),
            AsmInstr::Mov {
                src: AsmOp::Reg(AsmReg::R12),
                dst: AsmOp::Reg(AsmReg::Ax),
            },
        ]);

        body.push(AsmInstr::Newline);

        body.extend([
            AsmInstr::Comment("Release the alignment padding.".to_string()),
            AsmInstr::Bin {
                kind: AsmInstrBinKind::Add,
                lhs: AsmOp::Imm(8),
                rhs: AsmOp::Reg(AsmReg::Sp),
            },
            AsmInstr::Newline,
        ]);

        /*   */
        body.extend([
            AsmInstr::Comment("Restore registers and return to VM.".to_string()),
            AsmInstr::Pop(AsmOp::Reg(AsmReg::R9)),
            AsmInstr::Pop(AsmOp::Reg(AsmReg::R15)),
            AsmInstr::Pop(AsmOp::Reg(AsmReg::R14)),
            AsmInstr::Pop(AsmOp::Reg(AsmReg::R13)),
            AsmInstr::Pop(AsmOp::Reg(AsmReg::R12)),
            AsmInstr::Newline,
        ]);

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
            AsmInstr::Comment("%rcx = JIT_ERROR".into()),
            AsmInstr::Mov {
                src: AsmOp::Imm(JIT_ERROR as i64),
                dst: AsmOp::Reg(AsmReg::Cx),
            },
            AsmInstr::Newline,
            AsmInstr::Comment("%rdx = ValueTag".into()),
            AsmInstr::Mov {
                src: AsmOp::Imm(ValueTag::Void as u64 as i64),
                dst: AsmOp::Reg(AsmReg::Dx),
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
                msg: "empty machine code".to_string(),
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
