use std::collections::HashMap;

use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::FloatType;
use inkwell::values::{FloatValue, FunctionValue, IntValue, PointerValue};
use inkwell::FloatPredicate;

use super::error::{CodegenError, CodegenResult};
use super::symbols::SymbolTable;
use super::value::CgValue;

pub struct CodegenContext<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    pub symbols: SymbolTable<'ctx>,
    pub functions: HashMap<String, FunctionValue<'ctx>>,
    pub current_function: Option<FunctionValue<'ctx>>,
}

impl<'ctx> CodegenContext<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        Self {
            context,
            module: context.create_module(module_name),
            builder: context.create_builder(),
            symbols: SymbolTable::new(),
            functions: HashMap::new(),
            current_function: None,
        }
    }

    pub fn f64_type(&self) -> FloatType<'ctx> {
        self.context.f64_type()
    }

    pub fn bool_type(&self) -> inkwell::types::IntType<'ctx> {
        self.context.bool_type()
    }

    pub fn current_fn(&self) -> CodegenResult<FunctionValue<'ctx>> {
        self.current_function
            .ok_or_else(|| CodegenError::Unsupported("no hay funcion activa".to_string()))
    }

    pub fn push_scope(&mut self) {
        self.symbols.push_scope();
    }

    pub fn pop_scope(&mut self) {
        self.symbols.pop_scope();
    }

    pub fn create_entry_alloca(&self, function: FunctionValue<'ctx>, name: &str) -> CodegenResult<PointerValue<'ctx>> {
        let entry = function
            .get_first_basic_block()
            .ok_or_else(|| CodegenError::Unsupported("funcion sin bloque entry".to_string()))?;

        let alloca_builder = self.context.create_builder();
        if let Some(first) = entry.get_first_instruction() {
            alloca_builder.position_before(&first);
        } else {
            alloca_builder.position_at_end(entry);
        }

        alloca_builder
            .build_alloca(self.f64_type(), name)
            .map_err(|e| CodegenError::Builder(e.to_string()))
    }

    pub fn is_current_block_terminated(&self) -> bool {
        self.builder
            .get_insert_block()
            .and_then(|b| b.get_terminator())
            .is_some()
    }

    pub fn require_number(&self, value: CgValue<'ctx>) -> CodegenResult<FloatValue<'ctx>> {
        match value {
            CgValue::Number(v) => Ok(v),
            CgValue::Bool(v) => self
                .builder
                .build_unsigned_int_to_float(v, self.f64_type(), "bool_to_num")
                .map_err(|e| CodegenError::Builder(e.to_string())),
            CgValue::Void => Err(CodegenError::Unsupported(
                "se esperaba valor numerico y se obtuvo void".to_string(),
            )),
        }
    }

    pub fn require_bool(&self, value: CgValue<'ctx>) -> CodegenResult<IntValue<'ctx>> {
        match value {
            CgValue::Bool(v) => Ok(v),
            CgValue::Number(v) => self
                .builder
                .build_float_compare(
                    FloatPredicate::ONE,
                    v,
                    self.f64_type().const_float(0.0),
                    "num_to_bool",
                )
                .map_err(|e| CodegenError::Builder(e.to_string())),
            CgValue::Void => Err(CodegenError::Unsupported(
                "se esperaba condicion booleana y se obtuvo void".to_string(),
            )),
        }
    }

    pub fn ensure_merge_block(&self, function: FunctionValue<'ctx>, name: &str) -> BasicBlock<'ctx> {
        self.context.append_basic_block(function, name)
    }
}
