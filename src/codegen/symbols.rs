use std::collections::HashMap;
use inkwell::values::PointerValue;
use crate::semantic::HulkType;

/// Slot de variable en el stack frame LLVM.
/// Lleva el puntero al alloca + el HulkType del valor almacenado.
/// El HulkType determina qué tipo LLVM usar en build_load/build_store.
#[derive(Debug, Clone)]
pub struct VarSlot<'ctx> {
    pub ptr:     PointerValue<'ctx>,
    pub hulk_ty: HulkType,
}

#[derive(Debug, Default)]
pub struct SymbolTable<'ctx> {
    scopes: Vec<HashMap<String, VarSlot<'ctx>>>,
}

impl<'ctx> SymbolTable<'ctx> {
    pub fn new() -> Self {
        Self { scopes: vec![HashMap::new()] }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub fn insert(&mut self, name: String, slot: VarSlot<'ctx>) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, slot);
        }
    }

    /// Búsqueda léxica: del scope más interno al más externo.
    pub fn get(&self, name: &str) -> Option<&VarSlot<'ctx>> {
        for scope in self.scopes.iter().rev() {
            if let Some(slot) = scope.get(name) {
                return Some(slot);
            }
        }
        None
    }
}
