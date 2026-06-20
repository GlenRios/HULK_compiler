// src/semantic/symbol_table.rs

use std::collections::HashMap;
use super::type_system::HulkType;

// ─────────────────────────────────────────────────────────────────────────────
//  Tipo de símbolo
// ─────────────────────────────────────────────────────────────────────────────
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

impl Symbol {
    pub fn variable(name: impl Into<String>, ty: HulkType, mutable: bool) -> Self {
        Self { name: name.into(), kind: SymbolKind::Variable { ty, mutable } }
    }

    pub fn function(
        name: impl Into<String>,
        params: Vec<HulkType>,
        return_type: HulkType,
    ) -> Self {
        Self { name: name.into(), kind: SymbolKind::Function { params, return_type } }
    }

    pub fn type_sym(name: impl Into<String>) -> Self {
        Self { name: name.into(), kind: SymbolKind::Type }
    }

    pub fn protocol_sym(name: impl Into<String>) -> Self {
        Self { name: name.into(), kind: SymbolKind::Protocol }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  SymbolTable — scopes anidados con búsqueda léxica
//
//  El scope 0 es el global. Cada `push_scope` abre uno nuevo.
//  `lookup` busca del más interno al más externo.
// ─────────────────────────────────────────────────────────────────────────────
pub struct SymbolTable {
    scopes: Vec<HashMap<String, Symbol>>,
}

impl SymbolTable {
    pub fn new() -> Self {
        // Scope global preexistente
        Self { scopes: vec![HashMap::new()] }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        // Nunca eliminar el scope global
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Define un símbolo en el scope actual.
    /// Retorna `false` si ya existe en *este mismo* scope (redefinición).
    pub fn define(&mut self, name: impl Into<String>, symbol: Symbol) -> bool {
        let name = name.into();
        let scope = self.scopes.last_mut().unwrap();
        if scope.contains_key(&name) {
            return false;
        }
        scope.insert(name, symbol);
        true
    }

    /// Actualiza el tipo de una variable ya definida (para inferencia).
    pub fn update_type(&mut self, name: &str, new_ty: HulkType) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(sym) = scope.get_mut(name) {
                if let SymbolKind::Variable { ref mut ty, .. } = sym.kind {
                    *ty = new_ty;
                    return true;
                }
            }
        }
        false
    }

    /// Actualiza el tipo de retorno de una función (para inferencia de retorno).
    pub fn update_function_return(&mut self, name: &str, return_type: HulkType) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(sym) = scope.get_mut(name) {
                if let SymbolKind::Function { return_type: ref mut rt, .. } = sym.kind {
                    *rt = return_type;
                    return true;
                }
            }
        }
        false
    }

    /// Búsqueda léxica: del scope más interno al más externo.
    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(sym) = scope.get(name) {
                return Some(sym);
            }
        }
        None
    }

    /// ¿El nombre está definido en el scope *actual* (no en padres)?
    pub fn in_current_scope(&self, name: &str) -> bool {
        self.scopes.last().map_or(false, |s| s.contains_key(name))
    }

    /// Profundidad actual de anidamiento (0 = global)
    pub fn depth(&self) -> usize {
        self.scopes.len() - 1
    }
}