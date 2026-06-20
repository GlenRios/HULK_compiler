use std::collections::HashMap;
use inkwell::values::PointerValue;
use crate::semantic::HulkType;

/// Posición en memoria con tipo conocido.
/// Representa cualquier lvalue: variable local, campo de objeto, elemento de vector.
/// El HulkType determina qué tipo LLVM usar en build_load / build_store.
#[derive(Debug, Clone)]
pub struct Place<'ctx> {
    pub ptr:     PointerValue<'ctx>,
    pub hulk_ty: HulkType,
}

#[derive(Debug, Default)]
pub struct SymbolTable<'ctx> {
    scopes: Vec<HashMap<String, Place<'ctx>>>,
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

    pub fn insert(&mut self, name: String, place: Place<'ctx>) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, place);
        }
    }

    /// Búsqueda léxica: del scope más interno al más externo.
    pub fn get(&self, name: &str) -> Option<&Place<'ctx>> {
        for scope in self.scopes.iter().rev() {
            if let Some(place) = scope.get(name) {
                return Some(place);
            }
        }
        None
    }
}
