use crate::{
    QError, QErrorKind,
    codegen::{
        bytecode::r#gen::{BytecodeProg, Op},
        ir::r#gen::{IRInstr, IRProg},
    },
    parser::ast::{BinaryExpressionKind, Literal, Type, TypeInterner, UnaryExpressionKind},
    runtime::{
        jit::{
            JIT_CONTINUE, JIT_ERROR, JIT_RET, JIT_THRESHOLD, JitBlock, JitCompiler, JitExit,
            JitTrampoline,
        },
        rt::{RT, RtResult},
        value::{Value, ValueTag, print_value},
    },
    tokenizer::token::Span,
};

macro_rules! unop {
    ($self:expr, $op:tt) => {
        let obj = $self.pop();
        $self.push(($op obj).into());
    };
}

macro_rules! binop {
    ($self:expr, $op:tt) => {
        let b = $self.pop();
        let a = $self.pop();

        $self.push((a $op b).into());
    };
}

pub struct Vm<'types> {
    mem: Vec<u8>,
    stack: Vec<Value>,
    tos: usize,
    fp_stack: Vec<Frame>,
    ir: IRProg,
    func: usize,
    pc: usize,
    bytecode: BytecodeProg,
    fp: usize,
    strings: Vec<Box<[u8]>>,
    jit: Option<JitState>,
    types: &'types TypeInterner,
}

impl<'types> Vm<'types> {
    pub fn new(
        bytecode: BytecodeProg,
        ir: IRProg,
        enable_jit: bool,
        types: &'types TypeInterner,
    ) -> Result<Self, QError> {
        let entrypoint = bytecode
            .funcs
            .iter()
            .position(|f| f.name == "main")
            .ok_or_else(|| QError {
                kind: QErrorKind::Vm,
                code: 1,
                msg: "No 'main' fn.".into(),
                span: Span { start: 0, end: 0 },
            })?;

        let strings = bytecode.strings.iter().map(|s| encode_string(s)).collect();

        let jit = if enable_jit {
            Some(JitState::new(&bytecode, types)?)
        } else {
            None
        };

        let tos = bytecode.funcs[entrypoint].locals_count;

        let mut vm = Vm {
            mem: Vec::with_capacity(16 * 1024 * 1024),
            stack: vec![
                Value {
                    tag: ValueTag::Void,
                    payload: 0,
                };
                1024
            ],
            tos,
            fp_stack: Vec::new(),
            ir,
            func: entrypoint,
            pc: 0,
            fp: 0,
            strings,
            jit,
            bytecode,
            types,
        };

        for aggregate in &vm.bytecode.funcs[entrypoint].aggregates {
            let start = vm.mem.len();
            let end = start + aggregate.1.size;

            vm.mem.resize(end, 0);

            let slot = Value {
                tag: ValueTag::Array,
                payload: unsafe { vm.mem.as_mut_ptr().add(start) as u64 },
            };

            vm.stack[aggregate.1.slot] = slot;
        }

        Ok(vm)
    }

    #[inline(always)]
    fn push(&mut self, value: Value) {
        assert!(self.tos < self.stack.len(), "stack overflow");

        self.stack[self.tos] = value;
        self.tos += 1;
    }

    #[inline(always)]
    fn pop(&mut self) -> Value {
        assert!(self.tos > 0, "stack underflow");

        self.tos -= 1;
        self.stack[self.tos]
    }

    pub fn exec(&mut self) -> Result<Value, QError> {
        let r = self.exec_until(None);
        assert_eq!(self.tos, 0);

        r
    }

    ///  Run the Vm until the top-level function returns (`None`), or until
    ///  the `fp_stack` reaches a specified depth (`Some(depth)`) after a
    ///  function return.
    ///
    ///  The rt trampoline (`jit_call()`) uses this to execute a language-level
    ///  call and return control to native code once the called function is done,
    ///  without continuing execution of the suspended jitted caller.
    pub fn exec_until(&mut self, stop_depth: Option<usize>) -> Result<Value, QError> {
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
                        return Err(QError {
                            kind: QErrorKind::Vm,
                            msg: "native code errored out".to_string(),
                            code,
                            span: Span { start: 0, end: 0 },
                        });
                    }
                }
            }

            /* Normal interpreter path.  */
            let op = self.bytecode.funcs[self.func].body[self.pc];

            self.pc += 1;

            match op {
                Op::Const(konst) => self.handle_op_const(konst),
                Op::StrConst(idx) => self.handle_op_strconst(idx),
                Op::Binary(kind) => self.handle_op_binary(kind),
                Op::Unary(kind) => self.handle_op_unary(kind),
                Op::Print => self.handle_op_print()?,
                Op::Deepget(idx) => self.handle_op_deepget(idx),
                Op::Deepset(idx) => self.handle_op_deepset(idx),
                Op::Jz(target) => self.handle_op_jz(target),
                Op::Jmp(target) => self.handle_op_jmp(target),
                Op::Call { target, argc } => {
                    self.handle_op_call(target, argc);
                    continue;
                }
                Op::Cvt { src_ty, dst_ty } => self.handle_op_cvt(src_ty, dst_ty),
                Op::GetAddr { local } => self.handle_op_getaddr(local),
                Op::Load(ty) => self.handle_op_load(ty),
                Op::Store(ty) => self.handle_op_store(ty),
                Op::AddPtr { scale } => self.handle_op_addptr(scale),
                Op::CpyToOffset { local, offset, ty } => {
                    self.handle_op_cpy_to_offset(local, offset, ty)
                }
                Op::CpyFromOffset { local, offset, ty } => {
                    self.handle_op_cpy_from_offset(local, offset, ty)
                }
                Op::Ret | Op::RetVoid => {
                    let v = if matches!(op, Op::Ret) {
                        self.pop()
                    } else {
                        Value {
                            tag: ValueTag::Void,
                            payload: 0,
                        }
                    };

                    let r = self.finish_ret(v, stop_depth);

                    if cfg!(debug_assertions) {
                        eprintln!("op: {op:?}");
                        self.print_stack();
                    }

                    if let Some(v) = r {
                        return Ok(v);
                    }

                    continue;
                }
            };

            if cfg!(debug_assertions) {
                eprintln!("op: {op:?}");
                self.print_stack();
            }
        }
    }

    fn print_stack(&self) {
        eprint!("stack: [");

        for (idx, obj) in self.stack.iter().enumerate() {
            if idx == self.tos {
                break;
            }

            eprint!("{obj}");

            if idx != self.tos - 1 {
                eprint!(", ");
            }
        }

        eprintln!("]");
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
            None if stop_depth == Some(self.fp_stack.len()) => Some(self.pop()),
            None => None,
        }
    }

    pub fn invoke_from_jit(&mut self, target: usize, args: &[Value]) -> Result<Value, QError> {
        let old_func = self.func;
        let old_pc = self.pc;
        let old_fp = self.fp;

        let old_tos = self.tos;
        let old_depth = self.fp_stack.len();

        let f = self.ir.function(target).unwrap();
        let params = &f.params;

        if params.len() != args.len() {
            return Err(QError {
                kind: QErrorKind::Vm,
                msg: format!(
                    "argcount mismatch (got {}, expected {})",
                    args.len(),
                    params.len()
                ),
                code: RtResult::Internal as u32,
                span: Span { start: 0, end: 0 },
            });
        }

        for &arg in args {
            self.push(arg);
        }

        self.handle_op_call(target, args.len());

        let r = self.exec_until(Some(old_depth));

        self.tos = old_tos;
        self.fp_stack.truncate(old_depth);

        self.func = old_func;
        self.pc = old_pc;
        self.fp = old_fp;

        r
    }

    fn compile_region(&mut self, func: usize, start_pc: usize) -> Option<JitBlock> {
        let bytecode_func = &self.bytecode.funcs[func];

        let start_ir = bytecode_func.ir_map.get(start_pc)?;
        let end_ir = self.find_region_end(func, start_pc)?;

        let end_pc = bytecode_func
            .ir_map
            .iter()
            .position(|&ir_idx| ir_idx >= end_ir)
            .unwrap_or(bytecode_func.body.len());

        if cfg!(debug_assertions) {
            eprintln!(
                "[JIT] compiling func={} pc={} ir={}..{}",
                func, start_pc, start_ir, end_ir,
            );
        }

        let mut jit = JitCompiler::new(self.types);

        match jit.compile(&self.ir, func, *start_ir, end_ir, start_pc, end_pc) {
            Ok(Some(block)) => {
                if cfg!(debug_assertions) {
                    eprintln!("[JIT]: compilation successful");
                }

                Some(block)
            }

            Ok(None) => {
                if cfg!(debug_assertions) {
                    eprintln!("[JIT]: compilation skipped");
                }

                None
            }

            Err(e) => {
                if cfg!(debug_assertions) {
                    eprintln!("[JIT]: compilation failed: {e}");
                }

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

        if cfg!(debug_assertions) {
            eprintln!(
                "[JIT] EXECUTING {:p}, pc={}..{}",
                block.code.mem, block.start_pc, block.end_pc,
            );
        }

        let args = self.stack[self.fp..].as_ptr();
        let native_entrypoints = jit.native_entrypoints.as_ptr();
        let r = unsafe { entry(self as *mut Vm, &RT, args, body, end_pc, native_entrypoints) };

        if cfg!(debug_assertions) {
            eprintln!("[JIT] native returned {:?}", r);
        }

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

        let f = self.ir.function(func)?;
        let body = &f.body;

        match body.last()? {
            IRInstr::Ret { .. } => Some(body.len()),
            _ => None,
        }
    }

    fn handle_op_const(&mut self, n: Literal) {
        self.push(n.into());
    }

    fn handle_op_strconst(&mut self, idx: usize) {
        let ptr = self.strings[idx].as_ptr();
        self.push(Value {
            tag: ValueTag::Str,
            payload: ptr as u64,
        });
    }

    fn handle_op_unary(&mut self, kind: UnaryExpressionKind) {
        use UnaryExpressionKind::*;

        match kind {
            LogicalNot => self.handle_op_not(),
            BitwiseNot => self.handle_op_not(),
            Negate => self.handle_op_neg(),
            _ => unreachable!(),
        }
    }

    fn handle_op_binary(&mut self, kind: BinaryExpressionKind) {
        use BinaryExpressionKind::*;

        match kind {
            Add => self.handle_op_add(),
            Sub => self.handle_op_sub(),
            Mul => self.handle_op_mul(),
            Div => self.handle_op_div(),
            Rem => self.handle_op_rem(),
            BitwiseOr => self.handle_op_bitor(),
            BitwiseAnd => self.handle_op_bitand(),
            BitwiseXor => self.handle_op_bitxor(),
            Shl => self.handle_op_shl(),
            Shr => self.handle_op_shr(),
            Less => self.handle_op_lt(),
            LessEqual => self.handle_op_le(),
            Greater => self.handle_op_gt(),
            GreaterEqual => self.handle_op_ge(),
            Equal => self.handle_op_eq(),
            NotEqual => self.handle_op_ne(),
            _ => unreachable!("got: {kind:?}"),
        }
    }

    #[inline(always)]
    fn handle_op_add(&mut self) {
        binop!(self, +);
    }

    #[inline(always)]
    fn handle_op_sub(&mut self) {
        binop!(self, -);
    }

    #[inline(always)]
    fn handle_op_mul(&mut self) {
        binop!(self, *);
    }

    #[inline(always)]
    fn handle_op_div(&mut self) {
        binop!(self, /);
    }

    #[inline(always)]
    fn handle_op_rem(&mut self) {
        binop!(self, %);
    }

    #[inline(always)]
    fn handle_op_bitor(&mut self) {
        binop!(self, |);
    }

    #[inline(always)]
    fn handle_op_bitand(&mut self) {
        binop!(self, &);
    }

    #[inline(always)]
    fn handle_op_bitxor(&mut self) {
        binop!(self, ^);
    }

    #[inline(always)]
    fn handle_op_shl(&mut self) {
        binop!(self, <<);
    }

    #[inline(always)]
    fn handle_op_shr(&mut self) {
        binop!(self, >>);
    }

    #[inline(always)]
    fn handle_op_lt(&mut self) {
        binop!(self, <);
    }

    #[inline(always)]
    fn handle_op_gt(&mut self) {
        binop!(self, >);
    }

    #[inline(always)]
    fn handle_op_ge(&mut self) {
        binop!(self, >=);
    }

    #[inline(always)]
    fn handle_op_le(&mut self) {
        binop!(self, <=);
    }

    #[inline(always)]
    fn handle_op_ne(&mut self) {
        binop!(self, !=);
    }

    #[inline(always)]
    fn handle_op_eq(&mut self) {
        binop!(self, ==);
    }

    #[inline(always)]
    fn handle_op_neg(&mut self) {
        unop!(self, -);
    }

    #[inline(always)]
    fn handle_op_not(&mut self) {
        unop!(self, !);
    }

    fn handle_op_cvt(&mut self, src_ty: Type, dst_ty: Type) {
        let value = self.pop();
        self.push(value.cast(src_ty, dst_ty));
    }

    fn handle_op_print(&mut self) -> Result<(), QError> {
        let obj = self.pop();
        print_value(obj).map_err(|_| QError {
            kind: QErrorKind::Vm,
            msg: "print_value failed".to_string(),
            code: RtResult::Io as u32,
            span: Span { start: 0, end: 0 },
        })
    }

    fn handle_op_deepget(&mut self, idx: usize) {
        self.push(self.stack[self.fp + idx]);
    }

    fn handle_op_deepset(&mut self, idx: usize) {
        let obj = self.pop();
        self.stack[self.fp + idx] = obj;
    }

    #[inline(always)]
    fn handle_op_jz(&mut self, target: usize) {
        let cond = self.pop();

        if cond.payload == 0 {
            self.pc = target;
        }
    }

    #[inline(always)]
    fn handle_op_jmp(&mut self, target: usize) {
        self.pc = target;
    }

    fn handle_op_call(&mut self, target: usize, argc: usize) {
        let fp = self.tos - argc;

        self.fp_stack.push(Frame {
            func: self.func,
            pc: self.pc,
            ptr: self.fp,
            mem_base: self.mem.len(),
        });

        let locals = self.bytecode.funcs[target].locals_count;
        let end = fp + locals;

        self.func = target;
        self.pc = 0;
        self.fp = fp;

        self.stack[fp + argc..end].fill(Value {
            tag: ValueTag::Void,
            payload: 0,
        });

        self.tos = end;

        for aggregate in self.bytecode.funcs[self.func].aggregates.values() {
            let start = self.mem.len();
            let end = start + aggregate.size;

            self.mem.resize(end, 0);

            let ptr = unsafe { self.mem.as_mut_ptr().add(start) };

            self.stack[self.fp + aggregate.slot] = Value {
                tag: ValueTag::Array,
                payload: ptr as u64,
            };
        }
    }

    fn handle_op_getaddr(&mut self, local: usize) {
        let obj = self.stack[self.fp + local];

        let ptr = match obj.tag {
            tag if tag.is_aggregate() => obj.payload,
            _ => &mut self.stack[self.fp + local].payload as *mut u64 as u64,
        };

        self.push(Value {
            tag: ValueTag::Ptr,
            payload: ptr,
        });
    }

    fn handle_op_addptr(&mut self, scale: usize) {
        let index = self.pop();
        let base = self.pop();

        assert_eq!(base.tag, ValueTag::Ptr);

        let base = base.payload as *mut u8;
        let offset = index.payload as isize * scale as isize;

        let ptr = base.wrapping_offset(offset);

        self.push(Value {
            tag: ValueTag::Ptr,
            payload: ptr as u64,
        });
    }

    fn handle_op_load(&mut self, ty: Type) {
        let ptr = self.pop();
        assert_eq!(ptr.tag, ValueTag::Ptr);

        let size = ty.size_of(self.types);
        assert!(size <= 8, "aggregate loads need separate handling");

        let mut bytes = [0u8; 8];

        unsafe {
            std::ptr::copy_nonoverlapping(ptr.payload as *const u8, bytes.as_mut_ptr(), size);
        }

        let bits = u64::from_le_bytes(bytes);

        /* Sign-extend smaller ints.  */
        let payload = match ty {
            Type::Int8 => bits as i8 as i64 as u64,
            Type::Int16 => bits as i16 as i64 as u64,
            Type::Int32 => bits as i32 as i64 as u64,
            Type::Boolean => (bits != 0) as u64,
            _ => bits,
        };

        self.push(Value {
            tag: ty.into(),
            payload,
        });
    }

    fn handle_op_store(&mut self, ty: Type) {
        let src = self.pop();
        let dst = self.pop();
        assert_eq!(dst.tag, ValueTag::Ptr);

        let size = ty.size_of(self.types);
        assert!(size <= 8, "aggregate stores need separate handling");

        let src = src.payload.to_le_bytes();
        let src = src.as_ptr();

        let dst = dst.payload as *mut u8;

        unsafe {
            std::ptr::copy_nonoverlapping(src, dst, size);
        }
    }

    fn handle_op_cpy_to_offset(&mut self, local: usize, offset: usize, ty: Type) {
        let src = self.pop();

        let dst = self.stack[self.fp + local];
        assert!(dst.tag.is_aggregate());

        let size = ty.size_of(self.types);

        let src = src.payload.to_le_bytes();
        let src = src.as_ptr();

        let dst = dst.payload as *mut u8;
        let dst = dst.wrapping_offset(offset as isize);

        unsafe {
            std::ptr::copy_nonoverlapping(src, dst, size);
        }
    }

    fn handle_op_cpy_from_offset(&mut self, local: usize, offset: usize, ty: Type) {
        todo!()
    }

    fn handle_op_ret(&mut self, v: Value) -> Option<Value> {
        self.tos = self.fp;

        match self.fp_stack.pop() {
            Some(frame) => {
                self.mem.truncate(frame.mem_base);

                self.func = frame.func;
                self.pc = frame.pc;
                self.fp = frame.ptr;

                self.push(v);

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
    fn new(bytecode: &BytecodeProg, types: &TypeInterner) -> Result<Self, QError> {
        let shared_entry = JitCompiler::new(types)
            .compile_entry()
            .map_err(|e| QError {
                kind: QErrorKind::Vm,
                msg: e.msg,
                code: RtResult::Internal as u32,
                span: Span { start: 0, end: 0 },
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
    mem_base: usize,
}

fn encode_string(s: &str) -> Box<[u8]> {
    let mut buf = Vec::with_capacity(8 + s.len());

    buf.extend_from_slice(&(s.len() as u64).to_le_bytes());
    buf.extend_from_slice(s.as_bytes());

    buf.into_boxed_slice()
}
