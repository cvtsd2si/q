use crate::{
    bytecode::r#gen::{BytecodeProg, Op},
    ir::r#gen::{IRInstr, IRProg},
    parser::ast::Literal,
    runtime::{
        jit::{JIT_CONTINUE, JIT_ERROR, JIT_RET, JIT_THRESHOLD, JitBlock, JitCompiler, JitExit},
        rt::{RT, RtResult},
        value::{Value, ValueTag, print_value},
    },
};

macro_rules! binop_arithmetic {
    ($stack:expr, $op:tt) => {
        let b = $stack.pop().unwrap();
        let a = $stack.pop().unwrap();

        $stack.push(a $op b);
    };
}

pub struct Vm {
    stack: Vec<Value>,
    fp_stack: Vec<Frame>,
    ir: IRProg,
    func: usize,
    pc: usize,
    hotness: Vec<Vec<u32>>,
    jit_code: Vec<Vec<Option<JitBlock>>>,
    bytecode: BytecodeProg,
    fp: usize,
}

impl Vm {
    pub fn new(bytecode: BytecodeProg, ir: IRProg) -> Self {
        let entrypoint = match bytecode.funcs.iter().position(|f| f.name == "main") {
            Some(entrypoint) => entrypoint,
            None => panic!("no main fn"),
        };

        Vm {
            stack: Vec::new(),
            fp_stack: Vec::new(),
            ir,
            func: entrypoint,
            pc: 0,
            hotness: bytecode
                .funcs
                .iter()
                .map(|f| vec![0; f.body.len()])
                .collect(),
            jit_code: bytecode
                .funcs
                .iter()
                .map(|f| std::iter::repeat_with(|| None).take(f.body.len()).collect())
                .collect(),
            bytecode,
            fp: 0,
        }
    }

    pub fn exec(&mut self) -> Result<Value, VmError> {
        self.exec_until(None)
    }

    /*
     * Run the Vm until the top-level function returns (`None`), or until
     * the `fp_stack` reaches a specified depth (`Some(depth)`) after a
     * function return.
     *
     * The rt trampoline (`jit_call()`) uses this to execute a language-level
     * call and return control to native code once the called function is done,
     * without continuing execution of the suspended jitted caller.
     *
     * e.g.:
     *
     * If we have the program below:
     *
     * ```
     * fn bar() -> i64 {
     *     ret 42;
     * }
     *
     * fn foo() -> i64 {
     *     ret bar();
     * }
     *
     * fn main() -> i64 {
     *     ret foo();
     * }
     * ```
     *
     * If `main()` were jitted, and if it wanted to call `foo()`,
     * the control flow would look like below:
     *
     *  Native main()
     *      |
     *      | call jit_call()
     *      v
     *  Rust jit_call()
     *      |
     *      | vm.invoke_from_jit(foo, ...)
     *      v
     *     Vm
     *      |
     *      | Execute foo()
     *      v
     *     ???
     *
     * ...and then what?
     *
     * The question is: When should the Vm give the control back to the trampoline?
     * Obviously, the answer is: whenever `foo()` is done.
     *
     * But, what if `foo()` in turn called `fn bar()`?
     *
     * Should the Vm give the control back to the trampoline when `bar()` is done?  No,
     * becaus `foo()` still needs to finish up its execution.
     *
     * When `foo()` returns, should the Vm continue interpreting `main()`?  No,
     * because `main()` is already executing natively, and its machine code is
     * waiting for the trampoline to return.
     *
     * This is the problem that `fn exec_until()` solves.
     *
     * Let's finish up tracing the example code:
     *
     * |      Event             | `fp_stack.len()` | `stop_depth` | Stop? |
     * |------------------------|------------------|--------------|-------|
     * | before calling `foo()` | 0                | —            | —     |
     * | enter `foo()`          | 1                | 0            | No    |
     * | `foo()` calls `bar()`  | 2                | 0            | No    |
     * | `bar()` returns        | 1                | 0            | No    |
     * | `foo()` returns        | 0                | 0            | Yes   |
     *
     * This works because there is a symmetry between calling and returning
     * from a function -- each call pushes a new `Frame`, and each ret pops one.
     *
     * A subtle part worth understanding is, why does `exec_until()` pop the retval?
     *
     * Let's consider the program below:
     *
     * ```
     * fn add(a: i64, b: i64) -> i64 {
     *     ret a + b;
     * }
     *
     * fn main() -> i64 {
     *     let result = add(10, 20);
     *     ret result;
     * }
     * ```
     *
     * Normally, if both functions were interpreted, before entering `add()`, the
     * Vm would have pushed the args to `add()` on the stack:
     *
     * STACK:  20
     *         10
     *
     * Then `add()` executes which pops both values off the stack and pushes the
     * result:
     *
     * STACK:  30
     *
     * When it reaches `Op::Ret`, the handler for this opcode (`handle_op_ret`)
     * restores the caller's (`main()`'s) call stack, and the Vm can continue
     * executing `main()`.
     *
     * This behavior is exactly what we want for the interpreted caller.
     *
     * But, what if the caller were jitted?
     *
     * Native code expects the retval of `add()` to be written to `out: *mut Value`
     * from here:
     *
     * FIXME: add fn signature
     *
     * It does *NOT* expect the result to remain on the Vm's stack.
     *
     * `fn invoke_from_jit()` prepares the arguments:
     *
     *          ```
     *          self.stack.extend(values);
     *          self.handle_op_call(target, payloads.len());
     *          ```
     *
     * STACK:  20
     *         10 <-- `add()`s frame begins here
     *
     * ...then:
     *
     *         ```
     *         let r = self.exec_until(Some(old_depth));
     *         ```
     *
     * ...and the interpreter executes `add()`.  Upon `add()`'s return,
     * the interpreter reaches `Op::Ret`, pops the value off the stack,
     * and passes it to the handler (`handle_op_ret(v)`).  The handler
     * again restores the caller's (`main()`'s) stack, and pushes the
     * retval on the Vm's stack:
     *
     * STACK: 30
     *
     * Here comes the Sun.
     *
     * `handle_op_ret()` has restored `main()` as the current Vm fn.
     * But, `main()` is running natively, which means that we must
     * return to the native caller rather than keep on interpreting
     * it.
     *
     * So, `exec_until()` intercepts this return on the condition that
     * the function that the trampoline had requested has finished executing.
     * This retval needs returned to the trampoline, and it is conveniently
     * placed on the top of stack -- hence why the `self.stack.pop()`.
     *
     * What happens next?
     *
     * Well, the trampoline that had called `invoke_from_jit()` does:
     *
     *         ```
     *         *out = v;
     *         ```
     *
     * ...and voila, the result is moved from the interpreter's stack into
     * the native caller's retval storage.
     */
    pub fn exec_until(&mut self, stop_depth: Option<usize>) -> Result<Value, VmError> {
        loop {
            /* Have we jitted this instruction before?  */
            if self.jit_code[self.func][self.pc].is_some() {
                /* Yes, we have.  Pull it from the cache.  */
                match self.run_jitted() {
                    JitExit::Continue(pc) => {
                        self.pc = pc;
                        continue;
                    }

                    JitExit::Ret(v) => {
                        if let Some(v) = self.finish_ret(v, stop_depth) {
                            return Ok(v);
                        }

                        continue;
                    }

                    JitExit::Error(code) => {
                        return Err(VmError {
                            msg: format!("native code errored out"),
                            code,
                        });
                    }
                }
            }

            /* Mark this instruction a little hotter.  */
            self.hotness[self.func][self.pc] += 1;

            /* We have not jitted this instruction before.
             * If its hotness is above the `JIT_THRESHOLD`,
             * compile it into native code, cache it, and loop again.  */
            if self.hotness[self.func][self.pc] == JIT_THRESHOLD
                && let Some(block) = self.compile_region(self.func, self.pc)
            {
                self.jit_code[self.func][self.pc] = Some(block);
                continue;
            }

            /* Normal interpreter path.  */
            let op = self.bytecode.funcs[self.func].body[self.pc];

            match op {
                Op::Const(konst) => self.handle_op_const(konst),
                Op::Add => self.handle_op_add(),
                Op::Sub => self.handle_op_sub(),
                Op::Print => self.handle_op_print()?,
                Op::Deepget(idx) => self.handle_op_deepget(idx),
                Op::Deepset(idx) => self.handle_op_deepset(idx),
                Op::Jz(target) => self.handle_op_jz(target),
                Op::Jmp(target) => self.handle_op_jmp(target),
                Op::Call { target, argc } => {
                    self.handle_op_call(target, argc);
                    continue;
                }
                Op::Ret | Op::RetVoid => {
                    let v = if matches!(op, Op::Ret) {
                        self.stack.pop().unwrap()
                    } else {
                        Value {
                            tag: ValueTag::Void,
                            payload: 0,
                        }
                    };

                    if let Some(v) = self.finish_ret(v, stop_depth) {
                        return Ok(v);
                    }

                    continue;
                }
            };

            self.pc += 1;
        }
    }

    fn finish_ret(&mut self, v: Value, stop_depth: Option<usize>) -> Option<Value> {
        match self.handle_op_ret(v) {
            Some(v) => Some(v),
            None if stop_depth == Some(self.fp_stack.len()) => Some(self.stack.pop().unwrap()),
            None => None,
        }
    }

    pub fn invoke_from_jit(&mut self, target: usize, payloads: &[u64]) -> Result<Value, VmError> {
        let old_func = self.func;
        let old_pc = self.pc;
        let old_fp = self.fp;

        let old_stack_len = self.stack.len();
        let old_depth = self.fp_stack.len();

        let params = &self.ir.funcs[target].params;

        if params.len() != payloads.len() {
            return Err(VmError {
                msg: format!(
                    "argcount mismatch (got {}, expected {}",
                    payloads.len(),
                    params.len()
                ),
                code: RtResult::Internal as u32,
            });
        }

        let values = params
            .iter()
            .zip(payloads)
            .map(|(param, payload)| Value {
                tag: ValueTag::from(param.ty),
                payload: *payload,
            })
            .collect::<Vec<Value>>();

        self.stack.extend(values);

        self.handle_op_call(target, payloads.len());

        let r = self.exec_until(Some(old_depth));

        self.stack.truncate(old_stack_len);
        self.fp_stack.truncate(old_depth);

        self.func = old_func;
        self.pc = old_pc;
        self.fp = old_fp;

        r
    }

    fn compile_region(&self, func: usize, start_pc: usize) -> Option<JitBlock> {
        let bytecode_func = &self.bytecode.funcs[func];

        /* JIT compiler operates on the IR, so it needs IR coords.  */
        let start_ir = bytecode_func.ir_map.get(start_pc)?;
        let end_ir = self.find_region_end(func, start_pc)?;

        /* Find the first bytecode PC (for the Vm) that belongs to the IRInstr
         * at (or after) `end_ir`.
         *
         * If such a PC does not exist, bytecode length is taken.
         *
         * The indexes in the `ir_map` correspond to bytecode PCs.  */
        let end_pc = bytecode_func
            .ir_map
            .iter()
            .position(|&ir_idx| ir_idx >= end_ir)
            .unwrap_or(bytecode_func.body.len());

        eprintln!(
            "[JIT] compiling func={} pc={} ir={}..{}",
            func, start_pc, start_ir, end_ir,
        );

        /* Passing in PC coords, too, because the JIT compiler needs them
         * to embed them as metadata on `JitBlock`s it gives out.  */
        let mut jit = JitCompiler::new();

        match jit.compile(&self.ir, func, *start_ir, end_ir, start_pc, end_pc) {
            Ok(Some(block)) => {
                eprintln!("[JIT]: compilation successful");
                Some(block)
            }

            Ok(None) => {
                eprintln!("[JIT]: compilation skipped");
                None
            }

            Err(e) => {
                eprintln!("[JIT]: compilation failed: {e}");
                None
            }
        }
    }

    fn run_jitted(&mut self) -> JitExit {
        /* I think it is okay to unwrap here as we had checked
         * that this cannot be None.  */
        let block = self.jit_code[self.func][self.pc].as_ref().unwrap();

        let entry = block.entry;

        eprintln!(
            "[JIT] EXECUTING {:p}, pc={}..{}",
            block.mem, block.start_pc, block.end_pc,
        );

        /* Call into the generated machine code through the function ptr.  */
        let args = self.stack[self.fp..].as_ptr();
        let r = unsafe { entry(self as *mut Vm, &RT, args) };

        eprintln!("[JIT] native returned {:?}", r);

        match r.kind {
            JIT_CONTINUE => JitExit::Continue(r.next_pc as usize),
            JIT_RET => JitExit::Ret(r.value),
            JIT_ERROR => JitExit::Error(r.value.payload as u32),

            kind => panic!("unknown JIT exit kind: {kind}"),
        }
    }

    fn find_region_end(&self, func: usize, start_pc: usize) -> Option<usize> {
        if start_pc != 0 {
            return None;
        }

        let body = &self.ir.funcs.get(func)?.body;

        match body.last()? {
            IRInstr::Ret { .. } => Some(body.len()),
            _ => None,
        }
    }

    fn handle_op_const(&mut self, n: Literal) {
        self.stack.push(n.into());
    }

    fn handle_op_add(&mut self) {
        binop_arithmetic!(self.stack, +);
    }

    fn handle_op_sub(&mut self) {
        binop_arithmetic!(self.stack, -);
    }

    fn handle_op_print(&mut self) -> Result<(), VmError> {
        let obj = self.stack.pop().unwrap();
        print_value(obj).map_err(|_| VmError {
            msg: format!("print_value failed"),
            code: RtResult::Io as u32,
        })
    }

    fn handle_op_deepget(&mut self, idx: usize) {
        self.stack.push(self.stack[self.fp + idx]);
    }

    fn handle_op_deepset(&mut self, idx: usize) {
        let obj = self.stack.pop().unwrap();
        self.stack[self.fp + idx] = obj;
    }

    fn handle_op_jz(&mut self, target: usize) {
        let cond = self.stack.pop().unwrap();

        if cond.payload == 0 {
            self.pc = target - 1;
        }
    }

    fn handle_op_jmp(&mut self, target: usize) {
        self.pc = target - 1;
    }

    fn handle_op_call(&mut self, target: usize, argc: usize) {
        let fp = self.stack.len() - argc;

        self.fp_stack.push(Frame {
            func: self.func,
            pc: self.pc + 1,
            ptr: self.fp,
        });

        self.func = target;
        self.pc = 0;
        self.fp = fp;
    }

    fn handle_op_ret(&mut self, v: Value) -> Option<Value> {
        self.stack.truncate(self.fp);

        match self.fp_stack.pop() {
            Some(frame) => {
                self.func = frame.func;
                self.pc = frame.pc;
                self.fp = frame.ptr;

                self.stack.push(v);

                None
            }
            None => Some(v),
        }
    }
}

#[derive(Debug)]
struct Frame {
    func: usize,
    pc: usize,
    ptr: usize,
}

#[derive(Debug)]
pub struct VmError {
    pub msg: String,
    pub code: u32,
}

impl std::error::Error for VmError {}

impl std::fmt::Display for VmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "vm: {} (code: {})", self.msg, self.code)
    }
}
