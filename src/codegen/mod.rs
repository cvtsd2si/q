pub mod asm;
pub mod bytecode;
pub mod ir;

use crate::{
    QError,
    codegen::{bytecode::r#gen::BytecodeFunc, ir::r#gen::StaticInit},
    parser::ast::Type,
};

use std::{any::TypeId, collections::HashMap};

pub trait Codegen<T> {
    fn codegen(&self, ctx: &mut CodegenContext) -> Result<T, QError>;
}

#[derive(Debug)]
pub struct CodegenContext {
    pub current_fn: String,
    pub locals: HashMap<String, usize>,
    pub funcs: HashMap<String, usize>,
    pub static_consts: Vec<StaticConst>,
    pub float_pool: HashMap<(u8, u64), String>,
    /* Bytecode  */
    pub string_pool: Vec<String>,
    pub string_indexes: HashMap<String, usize>,
    /* ASM  */
    pub asm_string_pool: HashMap<String, String>,
}

impl Default for CodegenContext {
    fn default() -> Self {
        Self::new()
    }
}

impl CodegenContext {
    pub fn get<T: 'static>(&self, name: &str) -> Option<usize> {
        match TypeId::of::<T>() {
            id if id == TypeId::of::<String>() => self.locals.get(name).copied(),
            id if id == TypeId::of::<BytecodeFunc>() => self.funcs.get(name).copied(),
            _ => unreachable!(),
        }
    }

    pub fn register<T: 'static>(&mut self, name: &str) -> usize {
        let structure = match TypeId::of::<T>() {
            id if id == TypeId::of::<String>() => &mut self.locals,
            id if id == TypeId::of::<BytecodeFunc>() => &mut self.funcs,
            _ => unreachable!(),
        };

        if let Some(&idx) = structure.get(name) {
            return idx;
        }

        let idx = structure.len();

        structure.insert(name.to_owned(), idx);

        idx
    }

    pub fn intern_string(&mut self, s: &str) -> usize {
        if let Some(&idx) = self.string_indexes.get(s) {
            return idx;
        }

        let idx = self.string_pool.len();

        self.string_pool.push(s.to_owned());
        self.string_indexes.insert(s.to_owned(), idx);

        idx
    }
}

#[derive(Debug, Clone)]
pub struct StaticConst {
    pub name: String,
    pub init: StaticInit,
    pub ty: Type,
    pub alignment: usize,
}
