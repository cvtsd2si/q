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

pub struct JitCompiler {}

impl JitCompiler {
    pub fn new() -> Self {
        Self {}
    }

    pub fn compile(
        &mut self,
        ir: &IRProg,
        func: usize,
        /* What code to compile?  */
        start_ir: usize,
        end_ir: usize,
        /* What interpreted bytecode this native code replaces?  */
        start_pc: usize,
        end_pc: usize,
    ) -> Result<Option<JitBlock>, JitError> {
        if start_pc != 0 {
            return Ok(None);
        }

        let ir_func = ir.funcs.get(func).unwrap();

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

        /* JIT entry emission.  */
        let entry = AsmFunc {
            name: format!("{}_entry", region_func.name),
            body: self.mk_entry(&region_func.name, end_pc),
            space: 0,
        };
        entry.emit(&mut asm_txt).map_err(|_| JitError {
            msg: format!("jit entry emission failed"),
        })?;

        let mut ctx = CodegenContext::new();

        for func in ir.funcs.iter() {
            ctx.register::<BytecodeFunc>(&func.name);
        }

        /* Compiled fn emission.  */
        let mut asm_func: AsmFunc = region_func.codegen(&mut ctx).map_err(|_| JitError {
            msg: format!("jit codegen failed"),
        })?;
        let mut asm_func = asm_func.regalloc().fixup();

        /* Backpatch the amount of stack space required.  */
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

        asm_func.emit(&mut asm_txt).map_err(|e| JitError {
            msg: format!("jit code emission failed"),
        })?;

        /* Runtime error handler emission.  */
        let rt_err_handler = AsmFunc {
            name: format!("{}_rt_err", region_func.name),
            body: self.mk_rt_err(),
            space: 0,
        };
        rt_err_handler.emit(&mut asm_txt).map_err(|_| JitError {
            msg: format!("jit entry emission failed"),
        })?;

        eprintln!("[JIT ASM]\n{}", asm_txt);

        let arch = arch::lookup("x86-64").unwrap();
        let mut assembler = Assembler::new(arch, Options::new().with_relocatable(false));

        assembler.assemble_str("", &asm_txt);

        if !assembler.finish() {
            eprintln!(
                "{}",
                assembler.diags().render(assembler.source_map(), false)
            );

            return Err(JitError {
                msg: format!("JIT asm failed"),
            });
        }

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

            Ok(Some(JitBlock {
                entry,
                start_pc,
                end_pc,
                mem,
                len,
            }))
        }
    }

    fn mk_entry(&mut self, body_name: &str, end_pc: usize) -> Vec<AsmInstr> {
        /* SystemV AMD64 ABI (Linux x86-64)
         *
         * Section 1: argument passing
         *
         * According to the SystemV AMD64 ABI, the first six integer and ptr args
         * are passed to the called function in %rdi, %rsi, %rdx, %rcx, %r8, %r9.
         *
         * Floating point args use the SIMD registers (%xmm0 through %xmm7).
         *
         * Args that cannot be assigned the registers required by their ABI classification
         * are passed on the stack, where %rsp holds the return address of the caller, and
         * the first arg is at 8(%rsp).
         *
         * It is *NOT* the case that every arg smaller than 8 bytes goes into a reg,
         * and everything larger goes on the stack.  Rather, every arg is classified
         * by the arg type and the layout (INTEGER, SSE, MEMORY, etc.)
         *
         * Simple scalars (like ints and ptrs) are passed through general-purpose
         * registers, while floating point args are passed through SSE registers.
         *
         * Structures are divided into eightbytes and each eightbyte is classified
         * according to its arg type.
         *
         * Section 2: return values
         *
         * Return value is passed through %rax (or %eax) for the INTEGER class,
         * and in %xmm0 for the SSE class.  If the return value cannot fit in a
         * single register, additional registers are used (%rdx and %xmm1), respectively.
         * If the return value still cannot fit into these two registers, the caller
         * must allocate space for the returned struct and pass it as a ptr in %rdi.
         * Before returning, it places the result in %rax.
         *
         * Section 3: register preservation
         *
         * The SystemV AMD64 ABI also specifies which registers will survive the call
         * and not get clobbered by the called function.  There are two cases:
         *
         *  - callee saved registers:    these registers cannot be freely modified by the called
         *    (%rbx, %rbp, %r12-%r15)    function, and they must contain the same value when the
         *                               called function returns, as they had when it entered
         *
         *  - caller saved registers:    these registers can be freely clobbered by the called
         *    (%rax, %rcx, %rdx, %rdi,   function, and the caller must preserve them if it wants
         *     %rsi, %r8, %r9, %r10,     to use them past the call.
         *     %r11, %xmm0-%xmm15)
         *
         * Section 4: stack alignment
         *
         * Immediately before executing the `call` instr, the stack must be aligned to a 16-byte
         * boundary, that is %rsp must be divisible by 16.
         *
         * The `call` instr conceptually does:
         *      %rsp -= 8
         *      %mem[%rsp] = ret_addr
         *      %rip = addr_of(f)
         * When `f` executes `ret`, the processor pops the ret_addr from the stack and jumps to it.
         *
         * Section 5: variadic functions
         *
         * Variadic functions calls (e.g. `printf("%d", 42)`) introduce new rule, which is that
         * the ptr to `"%d"` is passed in %rdi, and %esi needs to contain `42`.
         * For calls that use a variable arg list, the caller must also set %al to contain an upper
         * bound (0-8) on the number of SSE regs used for passing args.
         */
        let mut body = vec![];

        insert_prologue(&mut body);

        /* Preserve callee-saved registers we use in our ABI.  */
        body.extend(vec![
            AsmInstr::Push(AsmOp::Reg(AsmReg::R12)),
            AsmInstr::Push(AsmOp::Reg(AsmReg::R13)),
            AsmInstr::Push(AsmOp::Reg(AsmReg::R14)),
        ]);

        /* Stack alignment for now.  */
        body.push(AsmInstr::Bin {
            kind: AsmInstrBinKind::Sub,
            lhs: AsmOp::Imm(8),
            rhs: AsmOp::Reg(AsmReg::Sp),
        });

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
        body.push(AsmInstr::Call {
            target: body_name.to_owned(),
        });

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
                dst: AsmOp::Mem(AsmReg::R12, 0),
            }, // kind
            AsmInstr::Mov {
                src: AsmOp::Imm(end_pc as i64),
                dst: AsmOp::Mem(AsmReg::R12, 8),
            }, // next_pc
            AsmInstr::Mov {
                src: AsmOp::Reg(AsmReg::Dx),
                dst: AsmOp::Mem(AsmReg::R12, 16),
            }, // value tag
            AsmInstr::Mov {
                src: AsmOp::Reg(AsmReg::Ax),
                dst: AsmOp::Mem(AsmReg::R12, 24),
            }, // payload
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

pub type JitEntry =
    unsafe extern "C" fn(*mut Vm, *const Rt, *const Value /* parameters  */) -> JitResult;

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

pub const JIT_CONTINUE: u64 = 0;
pub const JIT_RET: u64 = 1;
pub const JIT_ERROR: u64 = 2;

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
