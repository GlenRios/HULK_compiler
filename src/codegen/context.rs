use std::collections::HashMap;

use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::values::{FunctionValue, PointerValue};

use crate::parser::ast::TypeDecl;
use crate::semantic::{FuncSignature, HulkType, TypeHierarchy, SemanticOutput};
use super::error::{CodegenError, CodegenResult};
use super::objects::ObjectRegistry;
use super::symbols::{SymbolTable, Place};
use super::value::CgValue;

pub struct CodegenContext<'ctx> {
    pub context:             &'ctx Context,
    pub module:              Module<'ctx>,
    pub builder:             Builder<'ctx>,
    pub symbols:             SymbolTable<'ctx>,
    pub functions:           HashMap<String, FunctionValue<'ctx>>,
    pub func_sigs:           HashMap<String, FuncSignature>,
    pub current_function:    Option<FunctionValue<'ctx>>,
    pub type_hierarchy:      TypeHierarchy,
    pub type_registry:       ObjectRegistry<'ctx>,
    pub self_ptr:            Option<PointerValue<'ctx>>,
    pub current_type_name:   Option<String>,
    pub current_method_name: Option<String>,
    pub expr_types:          HashMap<u32, HulkType>,
    pub type_decls:          HashMap<String, TypeDecl>,
}

impl<'ctx> CodegenContext<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        Self {
            context,
            module:              context.create_module(module_name),
            builder:             context.create_builder(),
            symbols:             SymbolTable::new(),
            functions:           HashMap::new(),
            func_sigs:           HashMap::new(),
            current_function:    None,
            type_hierarchy:      TypeHierarchy::new(),
            type_registry:       ObjectRegistry::new(),
            self_ptr:            None,
            current_type_name:   None,
            current_method_name: None,
            expr_types:          HashMap::new(),
            type_decls:          HashMap::new(),
        }
    }

    pub fn from_semantic_output(
        context:     &'ctx Context,
        module_name: &str,
        output:      SemanticOutput,
    ) -> Self {
        Self {
            context,
            module:              context.create_module(module_name),
            builder:             context.create_builder(),
            symbols:             SymbolTable::new(),
            functions:           HashMap::new(),
            func_sigs:           output.functions,
            current_function:    None,
            type_hierarchy:      output.hierarchy,
            type_registry:       ObjectRegistry::new(),
            self_ptr:            None,
            current_type_name:   None,
            current_method_name: None,
            expr_types:          output.expr_types,
            type_decls:          HashMap::new(),
        }
    }

    // ── Estado de control ─────────────────────────────────────────────────────

    pub fn current_fn(&self) -> CodegenResult<FunctionValue<'ctx>> {
        self.current_function
            .ok_or_else(|| CodegenError::Unsupported("no hay funcion activa".to_string()))
    }

    pub fn push_scope(&mut self) { self.symbols.push_scope(); }
    pub fn pop_scope(&mut self)  { self.symbols.pop_scope();  }

    pub fn is_current_block_terminated(&self) -> bool {
        self.builder
            .get_insert_block()
            .and_then(|b| b.get_terminator())
            .is_some()
    }

    pub fn ensure_merge_block(&self, function: FunctionValue<'ctx>, name: &str) -> BasicBlock<'ctx> {
        self.context.append_basic_block(function, name)
    }

    // ── Alloca tipada ─────────────────────────────────────────────────────────

    pub fn create_entry_alloca_for(
        &self,
        function: FunctionValue<'ctx>,
        name:     &str,
        hulk_ty:  &HulkType,
    ) -> CodegenResult<Place<'ctx>> {
        let entry = function
            .get_first_basic_block()
            .ok_or_else(|| CodegenError::Unsupported("funcion sin bloque entry".to_string()))?;

        let ab = self.context.create_builder();
        if let Some(first) = entry.get_first_instruction() {
            ab.position_before(&first);
        } else {
            ab.position_at_end(entry);
        }

        let ptr = match hulk_ty {
            HulkType::Number  => ab.build_alloca(self.f64_type(),  name),
            HulkType::Boolean => ab.build_alloca(self.bool_type(), name),
            _                 => ab.build_alloca(self.ptr_type(),  name),
        }.map_err(|e| CodegenError::Builder(e.to_string()))?;

        Ok(Place { ptr, hulk_ty: hulk_ty.clone() })
    }

    pub fn create_entry_alloca(
        &self,
        function: FunctionValue<'ctx>,
        name:     &str,
    ) -> CodegenResult<PointerValue<'ctx>> {
        let entry = function
            .get_first_basic_block()
            .ok_or_else(|| CodegenError::Unsupported("funcion sin bloque entry".to_string()))?;

        let ab = self.context.create_builder();
        if let Some(first) = entry.get_first_instruction() {
            ab.position_before(&first);
        } else {
            ab.position_at_end(entry);
        }

        ab.build_alloca(self.f64_type(), name)
            .map_err(|e| CodegenError::Builder(e.to_string()))
    }

    // ── Load / Store tipados ──────────────────────────────────────────────────

    pub fn load_place(&self, place: &Place<'ctx>, name: &str) -> CodegenResult<CgValue<'ctx>> {
        match &place.hulk_ty {
            HulkType::Number => {
                let v = self.builder
                    .build_load(self.f64_type(), place.ptr, name)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .into_float_value();
                Ok(CgValue::Number(v))
            }
            HulkType::Boolean => {
                let v = self.builder
                    .build_load(self.bool_type(), place.ptr, name)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .into_int_value();
                Ok(CgValue::Bool(v))
            }
            HulkType::StringT => {
                let v = self.builder
                    .build_load(self.ptr_type(), place.ptr, name)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .into_pointer_value();
                Ok(CgValue::Str(v))
            }
            HulkType::Null => Ok(CgValue::Null),
            _ => {
                let v = self.builder
                    .build_load(self.ptr_type(), place.ptr, name)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .into_pointer_value();
                Ok(CgValue::Object(v))
            }
        }
    }

    pub fn store_place(&self, place: &Place<'ctx>, val: CgValue<'ctx>) -> CodegenResult<()> {
        match val {
            CgValue::Number(v) => {
                self.builder.build_store(place.ptr, v)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
            }
            CgValue::Bool(v) => {
                self.builder.build_store(place.ptr, v)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
            }
            CgValue::Str(v) | CgValue::Object(v) | CgValue::Vector(v) => {
                self.builder.build_store(place.ptr, v)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
            }
            CgValue::Null => {
                let null = self.ptr_type().const_null();
                self.builder.build_store(place.ptr, null)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
            }
            CgValue::Void => {}
        }
        Ok(())
    }
}
