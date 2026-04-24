use std::collections::HashMap;

use inkwell::AddressSpace;
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::{FloatType, IntType, PointerType};
use inkwell::values::{FloatValue, FunctionValue, IntValue, PointerValue};
use inkwell::FloatPredicate;

use crate::semantic::HulkType;
use super::error::{CodegenError, CodegenResult};
use super::symbols::{SymbolTable, VarSlot};
use super::value::CgValue;

pub struct CodegenContext<'ctx> {
    pub context:          &'ctx Context,
    pub module:           Module<'ctx>,
    pub builder:          Builder<'ctx>,
    pub symbols:          SymbolTable<'ctx>,
    pub functions:        HashMap<String, FunctionValue<'ctx>>,
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

    // ── Tipos LLVM básicos ────────────────────────────────────────────────────

    pub fn f64_type(&self) -> FloatType<'ctx> {
        self.context.f64_type()
    }

    pub fn bool_type(&self) -> IntType<'ctx> {
        self.context.bool_type()
    }

    /// Tipo puntero opaco (LLVM 17 opaque pointers).
    /// Usado para String, Object, Vector — todos son ptr en el IR.
    pub fn ptr_type(&self) -> PointerType<'ctx> {
        self.context.i8_type().ptr_type(AddressSpace::default())
    }

    // ── Control flow ──────────────────────────────────────────────────────────

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

    /// Crea un alloca en el bloque entry de la función con el tipo correcto
    /// según el HulkType. Reemplaza `create_entry_alloca` que era siempre f64.
    pub fn create_entry_alloca_for(
        &self,
        function: FunctionValue<'ctx>,
        name:     &str,
        hulk_ty:  &HulkType,
    ) -> CodegenResult<VarSlot<'ctx>> {
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

        Ok(VarSlot { ptr, hulk_ty: hulk_ty.clone() })
    }

    /// Versión legacy — sigue existiendo para código que todavía no migró.
    /// Siempre alloca f64 (funciones, parámetros numéricos).
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

    /// Carga un VarSlot y devuelve el CgValue correcto según su HulkType.
    /// El tipo LLVM del load debe coincidir con el tipo del alloca.
    pub fn load_slot(&self, slot: &VarSlot<'ctx>, name: &str) -> CodegenResult<CgValue<'ctx>> {
        match &slot.hulk_ty {
            HulkType::Number => {
                let v = self.builder
                    .build_load(self.f64_type(), slot.ptr, name)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .into_float_value();
                Ok(CgValue::Number(v))
            }
            HulkType::Boolean => {
                let v = self.builder
                    .build_load(self.bool_type(), slot.ptr, name)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .into_int_value();
                Ok(CgValue::Bool(v))
            }
            HulkType::StringT => {
                let v = self.builder
                    .build_load(self.ptr_type(), slot.ptr, name)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .into_pointer_value();
                Ok(CgValue::Str(v))
            }
            HulkType::Null => Ok(CgValue::Null),
            _ => {
                // Object, UserDefined, Protocol, Vector, Unknown
                let v = self.builder
                    .build_load(self.ptr_type(), slot.ptr, name)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .into_pointer_value();
                Ok(CgValue::Object(v))
            }
        }
    }

    /// Almacena un CgValue en un VarSlot.
    pub fn store_slot(&self, slot: &VarSlot<'ctx>, val: CgValue<'ctx>) -> CodegenResult<()> {
        match val {
            CgValue::Number(v) => {
                self.builder.build_store(slot.ptr, v)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
            }
            CgValue::Bool(v) => {
                self.builder.build_store(slot.ptr, v)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
            }
            CgValue::Str(v) | CgValue::Object(v) | CgValue::Vector(v) => {
                self.builder.build_store(slot.ptr, v)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
            }
            CgValue::Null => {
                let null = self.ptr_type().const_null();
                self.builder.build_store(slot.ptr, null)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
            }
            CgValue::Void => {} // nada que almacenar
        }
        Ok(())
    }

    // ── Coerciones ────────────────────────────────────────────────────────────

    /// Convierte cualquier CgValue a un i8* para pasarlo a hulk_print / concat.
    /// Números → hulk_str_from_number, strings → identity, otros → "null".
    pub fn cgvalue_to_str(&self, val: CgValue<'ctx>) -> CodegenResult<PointerValue<'ctx>> {
        match val {
            CgValue::Number(n) => {
                let f = self.module.get_function("hulk_str_from_number")
                    .ok_or_else(|| CodegenError::Unsupported("hulk_str_from_number no declarada".to_string()))?;
                Ok(self.builder
                    .build_call(f, &[n.into()], "num_to_str")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .try_as_basic_value().left().unwrap().into_pointer_value())
            }
            CgValue::Bool(b) => {
                // true → "true", false → "false" como global string
                let true_str  = self.builder.build_global_string_ptr("true",  "true_s")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?.as_pointer_value();
                let false_str = self.builder.build_global_string_ptr("false", "false_s")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?.as_pointer_value();
                Ok(self.builder
                    .build_select(b, true_str, false_str, "bool_str")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .into_pointer_value())
            }
            CgValue::Str(p)    => Ok(p),
            CgValue::Null      => {
                Ok(self.builder.build_global_string_ptr("null", "null_s")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?.as_pointer_value())
            }
            _ => Err(CodegenError::Unsupported("tipo no convertible a string todavia".to_string())),
        }
    }

    pub fn require_number(&self, value: CgValue<'ctx>) -> CodegenResult<FloatValue<'ctx>> {
        match value {
            CgValue::Number(v) => Ok(v),
            CgValue::Bool(v)   => self.builder
                .build_unsigned_int_to_float(v, self.f64_type(), "bool_to_num")
                .map_err(|e| CodegenError::Builder(e.to_string())),
            CgValue::Null      => Ok(self.f64_type().const_float(0.0)),
            CgValue::Void      => Err(CodegenError::Unsupported(
                "void en contexto numerico".to_string(),
            )),
            _ => Err(CodegenError::Unsupported(
                "tipo no numerico en contexto numerico".to_string(),
            )),
        }
    }

    pub fn require_bool(&self, value: CgValue<'ctx>) -> CodegenResult<IntValue<'ctx>> {
        match value {
            CgValue::Bool(v)   => Ok(v),
            CgValue::Number(v) => self.builder
                .build_float_compare(
                    FloatPredicate::ONE,
                    v,
                    self.f64_type().const_float(0.0),
                    "num_to_bool",
                )
                .map_err(|e| CodegenError::Builder(e.to_string())),
            CgValue::Null      => Ok(self.bool_type().const_int(0, false)),
            CgValue::Void      => Err(CodegenError::Unsupported(
                "void en contexto booleano".to_string(),
            )),
            _ => Err(CodegenError::Unsupported(
                "tipo no booleano en contexto booleano".to_string(),
            )),
        }
    }
}
