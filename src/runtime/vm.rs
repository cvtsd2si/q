use crate::{
    bytecode::r#gen::{BytecodeProg, Op},
    ir::r#gen::{CodegenContext, IRInstr, IRProg},
    parser::ast::Literal,
    runtime::{
        jit::{JIT_THRESHOLD, JitBlock, JitCompiler, JitExit},
        value::{Value, ValueTag},
    },
};

#[derive(Debug)]
struct Frame {
    func: usize,
    pc: usize,
    ptr: usize,
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

    pub fn exec(&mut self) -> Value {
        loop {
            /* Have we jitted this instruction before?  */
            if self.jit_code[self.func][self.pc].is_some() {
                /* Yes, we have.  Pull it from the cache.  */
                match self.run_jitted() {
                    JitExit::Continue(pc) => {
                        self.pc = pc;
                        continue;
                    }

                    JitExit::Ret(val) => {
                        if let Some(retval) = self.handle_op_ret(val) {
                            return retval;
                        }

                        continue;
                    }
                }
            }

            /* Mark this instruction a little hotter.  */
            self.hotness[self.func][self.pc] += 1;

            /* We have not jitted this instruction before.
             * If its hotness is above the `JIT_THRESHOLD`,
             * compile it into native code, cache it, and loop again.  */
            if self.hotness[self.func][self.pc] == JIT_THRESHOLD {
                if let Some(block) = self.compile_region(self.func, self.pc) {
                    self.jit_code[self.func][self.pc] = Some(block);
                    continue;
                }
            }

            /* Normal interpreter path.  */
            let op = self.bytecode.funcs[self.func].body[self.pc];

            match op {
                Op::Const(konst) => self.handle_op_const(konst),
                Op::Add => self.handle_op_add(),
                Op::Sub => self.handle_op_sub(),
                Op::Print => self.handle_op_print(),
                Op::Deepget(idx) => self.handle_op_deepget(idx),
                Op::Deepset(idx) => self.handle_op_deepset(idx),
                Op::Jz(target) => self.handle_op_jz(target),
                Op::Jmp(target) => self.handle_op_jmp(target),
                Op::Call { target, argc } => {
                    self.handle_op_call(target, argc);
                    continue;
                }
                Op::Ret => {
                    let v = self.stack.pop().unwrap();

                    match self.handle_op_ret(v) {
                        Some(retval) => return retval,
                        None => continue,
                    }
                }
                Op::RetVoid => {
                    let v = Value {
                        tag: ValueTag::Void,
                        payload: 0,
                    };

                    match self.handle_op_ret(v) {
                        Some(retval) => return retval,
                        None => continue,
                    }
                }
            };

            self.pc += 1;
        }
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
        JitCompiler::compile(&self.ir, func, *start_ir, end_ir, start_pc, end_pc)
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
        let r = unsafe { entry(self as *mut Vm) };

        eprintln!("[JIT] native returned {:?}", r);

        match r.kind {
            0 => JitExit::Continue(r.next_pc as usize),
            1 => JitExit::Ret(r.value),
            _ => unreachable!(),
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
        let b = self.stack.pop().unwrap();
        let a = self.stack.pop().unwrap();

        self.stack.push(a + b);
    }

    fn handle_op_sub(&mut self) {
        let b = self.stack.pop().unwrap();
        let a = self.stack.pop().unwrap();

        self.stack.push(a - b);
    }

    fn handle_op_print(&mut self) {
        let obj = self.stack.pop().unwrap();
        println!("{:?}", obj);
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
