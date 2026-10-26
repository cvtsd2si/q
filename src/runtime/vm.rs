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
    bytecode: BytecodeProg,
    fp: usize,
    jit: Option<JitState>,
}

impl Vm {
    pub fn new(bytecode: BytecodeProg, ir: IRProg, enable_jit: bool) -> Result<Self, VmError> {
        let entrypoint = match bytecode.funcs.iter().position(|f| f.name == "main") {
            Some(entrypoint) => entrypoint,
            None => panic!("no main fn"),
        };

        let jit = if enable_jit {
            Some(JitState::new(&bytecode)?)
        } else {
            None
        };

        Ok(Vm {
            stack: Vec::new(),
            fp_stack: Vec::new(),
            ir,
            func: entrypoint,
            pc: 0,
            fp: 0,
            jit,
            bytecode,
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
            if let Some(exit) = self.maybe_run_jit() {
                match exit {
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
                Op::UIntToDouble { src_ty } | Op::IntToDouble { src_ty } => {
                    self.handle_op_int_to_float(src_ty, Type::Float64)
                }
                Op::UIntToFloat { src_ty } | Op::IntToFloat { src_ty } => {
                    self.handle_op_int_to_float(src_ty, Type::Float32)
                }
                Op::DoubleToInt { dst_ty } | Op::DoubleToUInt { dst_ty } => {
                    self.handle_op_float_to_int(Type::Float64, dst_ty)
                }
                Op::FloatToInt { dst_ty } | Op::FloatToUInt { dst_ty } => {
                    self.handle_op_float_to_int(Type::Float32, dst_ty)
                }
                Op::DoubleToFloat => self.handle_op_float_to_float(Type::Float32),
                Op::FloatToDouble => self.handle_op_float_to_float(Type::Float64),
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

    fn maybe_run_jit(&mut self) -> Option<JitExit> {
        let func = self.func;
        let pc = self.pc;

        /* Return `None` if jit is disabled.  */
        let jit = self.jit.as_ref()?;

        /* Have we jitted this instruction before?  */
        if jit.code[func][pc].is_some() {
            /* Yes, we have.  */
            return Some(self.run_jitted());
        }

        let jit = self.jit.as_mut().unwrap();
        let hotness = &mut jit.hotness[func][pc];

        /* Mark this instruction a little hotter  */
        *hotness = hotness.saturating_add(1);

        /* Is it hot enough?  */
        if *hotness != JIT_THRESHOLD {
            /* No, it is not.  */
            return None;
        }

        /* Try compiling this region.  */
        let block = self.compile_region(func, pc)?;

        let jit = self.jit.as_mut().unwrap();

        /* Cache compiled code.  */
        if pc == 0 {
            jit.native_entrypoints[func] = block.code.as_ptr() as usize;
        }

        jit.code[func][pc] = Some(block);

        /* Exec the freshly compiled region immediately.  */
        Some(self.run_jitted())
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
        let jit = self.jit.as_ref().unwrap();

        let entry = jit.shared_entry.entry;

        let block = jit.code[self.func][self.pc].as_ref().unwrap();

        let body = block.code.as_ptr();
        let end_pc = block.end_pc;

        // eprintln!(
        //     "[JIT] EXECUTING {:p}, pc={}..{}",
        //     block.code.mem, block.start_pc, block.end_pc,
        // );

        let args = self.stack[self.fp..].as_ptr();
        let native_entrypoints = jit.native_entrypoints.as_ptr();
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

    fn handle_op_int_to_float(&mut self, src_ty: Type, dst_ty: Type) {
        let value = self.stack.pop().unwrap();

        let payload = match (src_ty.is_signed(), dst_ty) {
            (true, Type::Float32) => {
                (value.cast_int(Type::Int64).payload as i64 as f32).to_bits() as u64
            }
            (false, Type::Float32) => {
                (value.cast_int(Type::UInt64).payload as f32).to_bits() as u64
            }
            (true, Type::Float64) => (value.cast_int(Type::Int64).payload as i64 as f64).to_bits(),
            (false, Type::Float64) => (value.cast_int(Type::UInt64).payload as f64).to_bits(),
            _ => unreachable!("Invalid floating destination type: {dst_ty:?}"),
        };

        self.stack.push(Value {
            tag: ValueTag::from(dst_ty),
            payload,
        });
    }

    fn handle_op_float_to_float(&mut self, dst_ty: Type) {
        let value = self.stack.pop().unwrap();

        let payload = match (value.tag, dst_ty) {
            (ValueTag::Float64, Type::Float32) => {
                (f64::from_bits(value.payload) as f32).to_bits() as u64
            }
            (ValueTag::Float32, Type::Float64) => {
                (f32::from_bits(value.payload as u32) as f64).to_bits()
            }
            _ => unreachable!("Invalid float conversion: {:?} -> {dst_ty:?}", value.tag),
        };

        self.stack.push(Value {
            tag: ValueTag::from(dst_ty),
            payload,
        });
    }

    fn handle_op_float_to_int(&mut self, src_ty: Type, dst_ty: Type) {
        let value = self.stack.pop().unwrap();

        let n = match src_ty {
            Type::Float32 => f32::from_bits(value.payload as u32) as f64,
            Type::Float64 => f64::from_bits(value.payload),
            _ => unreachable!("Invalid floating source type: {src_ty:?}"),
        };

        let intermediate = match dst_ty {
            Type::Int8 | Type::UInt8 | Type::Int16 | Type::UInt16 | Type::Int32 => Value {
                tag: ValueTag::Int32,
                payload: Self::cvtt_i32(n) as i64 as u64,
            },
            Type::UInt32 | Type::Int64 => Value {
                tag: ValueTag::Int64,
                payload: Self::cvtt_i64(n) as u64,
            },
            Type::UInt64 => {
                const TWO_63: f64 = 9_223_372_036_854_775_808.0;
                let payload = if n.is_nan() || n < TWO_63 {
                    Self::cvtt_i64(n) as u64
                } else {
                    (Self::cvtt_i64(n - TWO_63) as u64).wrapping_add(1u64 << 63)
                };
                Value {
                    tag: ValueTag::UInt64,
                    payload,
                }
            }
            _ => unreachable!("Invalid integer destination type: {dst_ty:?}"),
        };

        self.stack.push(intermediate.cast_int(dst_ty));
    }

    fn cvtt_i32(n: f64) -> i32 {
        let n = n.trunc();
        if !n.is_finite() || n < -2_147_483_648.0 || n >= 2_147_483_648.0 {
            i32::MIN
        } else {
            n as i32
        }
    }

    fn cvtt_i64(n: f64) -> i64 {
        const TWO_63: f64 = 9_223_372_036_854_775_808.0;
        let n = n.trunc();
        if !n.is_finite() || n < -TWO_63 || n >= TWO_63 {
            i64::MIN
        } else {
            n as i64
        }
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

struct JitState {
    hotness: Vec<Vec<u32>>,
    code: Vec<Vec<Option<JitBlock>>>,
    native_entrypoints: Box<[usize]>,
    shared_entry: JitTrampoline,
}

impl JitState {
    fn new(bytecode: &BytecodeProg) -> Result<Self, VmError> {
        let shared_entry = JitCompiler::new().compile_entry().map_err(|e| VmError {
            msg: e.msg,
            code: RtResult::Internal as u32,
        })?;

        Ok(Self {
            hotness: bytecode
                .funcs
                .iter()
                .map(|f| vec![0; f.body.len()])
                .collect(),

            code: bytecode
                .funcs
                .iter()
                .map(|f| std::iter::repeat_with(|| None).take(f.body.len()).collect())
                .collect(),

            native_entrypoints: vec![0usize; bytecode.funcs.len()].into_boxed_slice(),

            shared_entry,
        })
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
