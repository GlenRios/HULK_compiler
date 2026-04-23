use std::collections::HashMap;

use inkwell::values::PointerValue;

#[derive(Debug, Default)]
pub struct SymbolTable<'ctx> {
    scopes: Vec<HashMap<String, PointerValue<'ctx>>>,
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
            let _ = self.scopes.pop();
        }
    }

    pub fn insert(&mut self, name: String, ptr: PointerValue<'ctx>) {
        if let Some(last) = self.scopes.last_mut() {
            last.insert(name, ptr);
        }
    }

    pub fn get(&self, name: &str) -> Option<PointerValue<'ctx>> {
        for scope in self.scopes.iter().rev() {
            if let Some(ptr) = scope.get(name) {
                return Some(*ptr);
            }
        }
        None
    }
}
