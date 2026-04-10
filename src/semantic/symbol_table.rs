use std::collections::HashMap;
use super::type_system::HulkType;

#[derive(Debug, Clone)]
pub enum SymbolKind {
    Variable { ty: HulkType, mutable: bool },
    Function { params: Vec<HulkType>, return_type: HulkType },
    Type,
    Protocol,
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
}

/// Tabla de símbolos con scopes anidados
pub struct SymbolTable {
    scopes: Vec<HashMap<String, Symbol>>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self { scopes: vec![HashMap::new()] } // scope global
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn define(&mut self, name: impl Into<String>, symbol: Symbol) -> bool {
        let name = name.into();
        let scope = self.scopes.last_mut().unwrap();
        if scope.contains_key(&name) {
            return false; // redefinición en mismo scope
        }
        scope.insert(name, symbol);
        true
    }

    /// Búsqueda léxica — del scope más interno al más externo
    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(sym) = scope.get(name) {
                return Some(sym);
            }
        }
        None
    }

    pub fn lookup_mut(&mut self, name: &str) -> Option<&mut Symbol> {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(sym) = scope.get_mut(name) {
                return Some(sym);
            }
        }
        None
    }

    pub fn in_current_scope(&self, name: &str) -> bool {
        self.scopes.last().map_or(false, |s| s.contains_key(name))
    }
}