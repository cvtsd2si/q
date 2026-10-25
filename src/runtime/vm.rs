use crate::{
    bytecode::r#gen::{BytecodeProg, Op},
    ir::r#gen::{IRInstr, IRProg},
    parser::ast::{Literal, Type},
    runtime::{
        jit::{
            JIT_CONTINUE, JIT_ERROR, JIT_RET, JIT_THRESHOLD, JitBlock, JitCompiler, JitExit,
            JitTrampoline,
        },
        rt::{RT, RtResult},
        value::{Value, ValueTag, print_value},
    },
};

macro_rules! unop {
    ($stack:expr, $op:tt) => {
        let obj = $stack.pop().unwrap();
        $stack.push(($op obj).into());
    };
}

macro_rules! binop {
    ($stack:expr, $op:tt) => {
        let b = $stack.pop().unwrap();
        let a = $stack.pop().unwrap();

        $stack.push((a $op b).into());
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
    native_entrypoints: Box<[usize]>,
    bytecode: BytecodeProg,
    fp: usize,
    shared_entry: JitTrampoline,
}

impl Vm {
    pub fn new(bytecode: BytecodeProg, ir: IRProg) -> Result<Self, VmError> {
        let entrypoint = match bytecode.funcs.iter().position(|f| f.name == "main") {
            Some(entrypoint) => entrypoint,
            None => panic!("no main fn"),
        };

        Ok(Vm {
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
            native_entrypoints: vec![0usize; bytecode.funcs.len()].into_boxed_slice(),
            bytecode,
            fp: 0,
            shared_entry: JitCompiler::new().compile_entry().map_err(|e| VmError {
                msg: e.msg,
                code: RtResult::Internal as u32,
            })?, // FIXME: this disallows full interpretation
        })
    }

    pub fn exec(&mut self) -> Result<Value, VmError> {
        self.exec_until(None)
    }

    ///  Run the Vm until the top-level function returns (`None`), or until
    ///  the `fp_stack` reaches a specified depth (`Some(depth)`) after a
    ///  function return.
    ///
    ///  The rt trampoline (`jit_call()`) uses this to execute a language-level
    ///  call and return control to native code once the called function is done,
    ///  without continuing execution of the suspended jitted caller.
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
                            msg: "native code errored out".to_string(),
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
                if self.pc == 0 {
                    self.native_entrypoints[self.func] = block.code.as_ptr() as usize;
                }

                self.jit_code[self.func][self.pc] = Some(block);

                continue;
            }

            /* Normal interpreter path.  */
            let op = self.bytecode.funcs[self.func].body[self.pc];

            match op {
                Op::Const(konst) => self.handle_op_const(konst),
                Op::Add => self.handle_op_add(),
                Op::Sub => self.handle_op_sub(),
                Op::L => self.handle_op_l(),
                Op::G => self.handle_op_g(),
                Op::Le => self.handle_op_le(),
                Op::Ge => self.handle_op_ge(),
                Op::E => self.handle_op_e(),
                Op::Ne => self.handle_op_ne(),
                Op::Neg => self.handle_op_neg(),
                Op::Not | Op::BitwiseNot => self.handle_op_not(),
                Op::Trunc { src_ty, dst_ty } => self.handle_op_trunc(src_ty, dst_ty),
                Op::Sx { src_ty, dst_ty } => self.handle_op_sx(src_ty, dst_ty),
                Op::Zx { src_ty, dst_ty } => self.handle_op_zx(src_ty, dst_ty),
                Op::UIntToDouble { src_ty } => self.handle_op_uint_to_double(src_ty),
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

    pub fn invoke_from_jit(&mut self, target: usize, args: &[Value]) -> Result<Value, VmError> {
        let old_func = self.func;
        let old_pc = self.pc;
        let old_fp = self.fp;

        let old_stack_len = self.stack.len();
        let old_depth = self.fp_stack.len();

        let params = &self.ir.funcs[target].params;

        if params.len() != args.len() {
            return Err(VmError {
                msg: format!(
                    "argcount mismatch (got {}, expected {}",
                    args.len(),
                    params.len()
                ),
                code: RtResult::Internal as u32,
            });
        }

        self.stack.extend(args);

        self.handle_op_call(target, args.len());

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

        let start_ir = bytecode_func.ir_map.get(start_pc)?;
        let end_ir = self.find_region_end(func, start_pc)?;

        let end_pc = bytecode_func
            .ir_map
            .iter()
            .position(|&ir_idx| ir_idx >= end_ir)
            .unwrap_or(bytecode_func.body.len());

        // eprintln!(
        //     "[JIT] compiling func={} pc={} ir={}..{}",
        //     func, start_pc, start_ir, end_ir,
        // );

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
        let entry = self.shared_entry.entry;

        let block = self.jit_code[self.func][self.pc].as_ref().unwrap();

        let body = block.code.as_ptr();
        let end_pc = block.end_pc;

        // eprintln!(
        //     "[JIT] EXECUTING {:p}, pc={}..{}",
        //     block.code.mem, block.start_pc, block.end_pc,
        // );

        let args = self.stack[self.fp..].as_ptr();
        let native_entrypoints = self.native_entrypoints.as_ptr();
        let r = unsafe { entry(self as *mut Vm, &RT, args, body, end_pc, native_entrypoints) };

        // eprintln!("[JIT] native returned {:?}", r);

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
        binop!(self.stack, +);
    }

    fn handle_op_sub(&mut self) {
        binop!(self.stack, -);
    }

    fn handle_op_l(&mut self) {
        binop!(self.stack, <);
    }

    fn handle_op_g(&mut self) {
        binop!(self.stack, >);
    }

    fn handle_op_le(&mut self) {
        binop!(self.stack, <=);
    }

    fn handle_op_ge(&mut self) {
        binop!(self.stack, >=);
    }

    fn handle_op_e(&mut self) {
        binop!(self.stack, ==);
    }

    fn handle_op_ne(&mut self) {
        binop!(self.stack, !=);
    }

    fn handle_op_neg(&mut self) {
        unop!(self.stack, -);
    }

    fn handle_op_not(&mut self) {
        unop!(self.stack, !);
    }

    fn handle_op_trunc(&mut self, _src_ty: Type, dst_ty: Type) {
        let value = self.stack.pop().unwrap();
        self.stack.push(value.cast_int(dst_ty));
    }

    fn handle_op_sx(&mut self, _src_ty: Type, dst_ty: Type) {
        let value = self.stack.pop().unwrap();
        self.stack.push(value.cast_int(dst_ty));
    }
    fn handle_op_zx(&mut self, _src_ty: Type, dst_ty: Type) {
        let value = self.stack.pop().unwrap();
        self.stack.push(value.cast_int(dst_ty));
    }

    fn handle_op_uint_to_double(&mut self, src_ty: Type) {
        /* pop the unsigned int  */
        let value = self.stack.pop().unwrap();

        /* interpret the payload according to `src_ty`  */
        let r = match src_ty {
            Type::UInt8 => value.payload as u8 as f64,
            Type::UInt16 => value.payload as u16 as f64,
            Type::UInt32 => value.payload as u32 as f64,
            Type::UInt64 => value.payload as f64,
            _ => unreachable!("Invalid unsigned source type: {src_ty:?}"),
        };

        /* push a new value tagged as `Float64`  */
        self.stack.push(Value {
            tag: ValueTag::Float64,
            payload: r.to_bits(),
        });
    }

    fn handle_op_print(&mut self) -> Result<(), VmError> {
        let obj = self.stack.pop().unwrap();
        print_value(obj).map_err(|_| VmError {
            msg: "print_value failed".to_string(),
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
