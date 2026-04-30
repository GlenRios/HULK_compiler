use std::collections::HashMap;
use inkwell::types::StructType;
use inkwell::values::{FunctionValue, GlobalValue};

pub struct TypeLayout<'ctx> {
    pub struct_type:   StructType<'ctx>,
    pub vtable_type:   StructType<'ctx>,
    pub vtable_global: GlobalValue<'ctx>,
    pub field_names:   Vec<String>,
    pub method_names:  Vec<String>,
    pub type_tag:      u32,  // tag mínimo del rango DFS — identifica el tipo
    pub max_tag:       u32,  // tag máximo del rango DFS — cubre todos los subtipos
    pub ctor_fn:       Option<FunctionValue<'ctx>>,
    pub parent:        Option<String>,
}

pub struct ObjectRegistry<'ctx> {
    pub layouts:  HashMap<String, TypeLayout<'ctx>>,
    next_tag:     u32,
}

impl<'ctx> ObjectRegistry<'ctx> {
    pub fn new() -> Self {
        Self { layouts: HashMap::new(), next_tag: 1 }
    }

    pub fn alloc_tag(&mut self) -> u32 {
        let t = self.next_tag;
        self.next_tag += 1;
        t
    }

    /// Índice LLVM del campo dentro del struct: 2 + posición en field_names.
    /// Los primeros dos campos son siempre type_tag (0) y vtable_ptr (1).
    pub fn field_llvm_index(&self, type_name: &str, field: &str) -> Option<u32> {
        let i = self.layouts.get(type_name)?.field_names.iter().position(|n| n == field)?;
        Some(2 + i as u32)
    }

    /// Slot del vtable para un método (índice en method_names).
    pub fn method_slot(&self, type_name: &str, method: &str) -> Option<u32> {
        self.layouts.get(type_name)?
            .method_names.iter().position(|n| n == method)
            .map(|i| i as u32)
    }
}
