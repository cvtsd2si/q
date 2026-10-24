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
        if let Some(instr) = asm_func.body.get_mut(2) {
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
        let rt_err_handler = AsmFunc {
            name: format!("{}_rt_err", region_func.name),
            body: self.mk_rt_err(),
            space: 0,
        };
        rt_err_handler.emit(&mut asm_txt).map_err(|_| JitError {
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

        /* Preserve callee-saved registers we use in our ABI.  */
        body.extend(vec![
            AsmInstr::Push(AsmOp::Reg(AsmReg::R12)),
            AsmInstr::Push(AsmOp::Reg(AsmReg::R13)),
            AsmInstr::Push(AsmOp::Reg(AsmReg::R14)),
            AsmInstr::Push(AsmOp::Reg(AsmReg::R9)),
        ]);

        /*
         * %rdi = hidden *mut JitResult ptr
         * %rsi = *mut Vm
         * %rdx = *const Rt
         * %rcx = *const Value (args)
         */
        body.extend(vec![
            AsmInstr::Mov {
                src: AsmOp::Reg(AsmReg::Di),
                dst: AsmOp::Reg(AsmReg::R12),
            },
            AsmInstr::Mov {
                src: AsmOp::Reg(AsmReg::Si),
                dst: AsmOp::Reg(AsmReg::R13),
            },
            AsmInstr::Mov {
                src: AsmOp::Reg(AsmReg::Dx),
                dst: AsmOp::Reg(AsmReg::R14),
            },
            AsmInstr::Mov {
                src: AsmOp::Reg(AsmReg::Cx),
                dst: AsmOp::Reg(AsmReg::Di),
            },
        ]);

        /* Execute the compiled region.  */
        body.push(AsmInstr::CallIndirect { target: AsmReg::R8 });

        /* According to the internal ABI, after the call instr,
         * %rcx will have contained the exit kind (one of CONTINUE,
         * RET, ERROR), while %rdx will have contained the ValueTag,
         * and %rax will have contained the payload or the err code.
         */

        /* The ptr to `JitResult` struct is in %r12.  */

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
        body.extend(vec![
            AsmInstr::Mov {
                src: AsmOp::Reg(AsmReg::Cx),
                dst: AsmOp::Mem(AsmReg::R12, std::mem::offset_of!(JitResult, kind) as isize),
            },
            AsmInstr::Mov {
                src: AsmOp::Mem(AsmReg::Sp, 0),
                dst: AsmOp::Reg(AsmReg::R10),
            },
            AsmInstr::Mov {
                src: AsmOp::Reg(AsmReg::R10),
                dst: AsmOp::Mem(
                    AsmReg::R12,
                    std::mem::offset_of!(JitResult, next_pc) as isize,
                ),
            },
            AsmInstr::Mov {
                src: AsmOp::Reg(AsmReg::Dx),
                dst: AsmOp::Mem(
                    AsmReg::R12,
                    (std::mem::offset_of!(JitResult, value) + std::mem::offset_of!(Value, tag))
                        as isize,
                ),
            },
            AsmInstr::Mov {
                src: AsmOp::Reg(AsmReg::Ax),
                dst: AsmOp::Mem(
                    AsmReg::R12,
                    (std::mem::offset_of!(JitResult, value) + std::mem::offset_of!(Value, payload))
                        as isize,
                ),
            },
            AsmInstr::Mov {
                src: AsmOp::Reg(AsmReg::R12),
                dst: AsmOp::Reg(AsmReg::Ax),
            },
        ]);

        /* Stack alignment for now.  */
        body.push(AsmInstr::Bin {
            kind: AsmInstrBinKind::Add,
            lhs: AsmOp::Imm(8),
            rhs: AsmOp::Reg(AsmReg::Sp),
        });

        /* Restore the native stack/register state and return to VM.  */
        body.extend(vec![
            AsmInstr::Pop(AsmOp::Reg(AsmReg::R14)),
            AsmInstr::Pop(AsmOp::Reg(AsmReg::R13)),
            AsmInstr::Pop(AsmOp::Reg(AsmReg::R12)),
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
            AsmInstr::Mov {
                src: AsmOp::Imm(JIT_ERROR as i64),
                dst: AsmOp::Reg(AsmReg::Cx),
            },
            AsmInstr::Mov {
                src: AsmOp::Imm(ValueTag::Void as u64 as i64),
                dst: AsmOp::Reg(AsmReg::Dx),
            },
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
