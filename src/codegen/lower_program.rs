use std::collections::HashMap;

use inkwell::AddressSpace;
use inkwell::types::BasicTypeEnum;

use crate::parser::ast::{Decl, Program, TypeDecl, TypeMember};

use super::context::CodegenContext;
use super::error::{CodegenError, CodegenResult};
use super::objects::TypeLayout;
use super::visitor::{DeclVisitor, ExprVisitor, ProgramVisitor};

impl<'ctx> ProgramVisitor<'ctx> for CodegenContext<'ctx> {
    fn visit_program(&mut self, program: &Program) -> CodegenResult<()> {
        self.register_runtime();
        self.predeclare_functions(&program.declarations);
        self.build_type_layouts(&program.declarations)?;

        for decl in &program.declarations {
            self.visit_decl(decl)?;
        }

        let entry_fn = self
            .module
            .add_function("__hulk_entry", self.f64_type().fn_type(&[], false), None);
        let entry_block = self.context.append_basic_block(entry_fn, "entry");

        self.current_function = Some(entry_fn);
        self.builder.position_at_end(entry_block);
        self.push_scope();

        let entry_value = self.visit_expr(&program.entry)?;
        let ret = self.require_number(entry_value)?;

        if !self.is_current_block_terminated() {
            self.builder
                .build_return(Some(&ret))
                .map_err(|e| CodegenError::Builder(e.to_string()))?;
        }

        self.pop_scope();
        self.current_function = None;

        self.module
            .verify()
            .map_err(|e| CodegenError::Verify(e.to_string()))?;

        Ok(())
    }
}

impl<'ctx> CodegenContext<'ctx> {
    pub fn build_type_layouts(&mut self, decls: &[Decl]) -> CodegenResult<()> {
        let ast_map: HashMap<String, &TypeDecl> = decls.iter()
            .filter_map(|d| if let Decl::Type(t) = d { Some((t.name.clone(), t)) } else { None })
            .collect();

        if ast_map.is_empty() {
            return Ok(());
        }

        // ── Pase 1: structs opacos ────────────────────────────────────────────
        // Inkwell no tiene module.get_struct_type() — guardamos en HashMaps locales
        let mut struct_map: HashMap<String, inkwell::types::StructType<'ctx>> = HashMap::new();
        let mut vtable_map: HashMap<String, inkwell::types::StructType<'ctx>> = HashMap::new();

        for decl in decls {
            if let Decl::Type(td) = decl {
                struct_map.insert(td.name.clone(),
                    self.context.opaque_struct_type(&td.name));
                vtable_map.insert(td.name.clone(),
                    self.context.opaque_struct_type(&format!("VTable_{}", td.name)));
            }
        }

        // ── Pase 1b: asignar tags en orden DFS para que is pueda usar range check ──
        // Construir mapa padre → hijos dentro del conjunto de tipos declarados
        let mut children: HashMap<String, Vec<String>> = HashMap::new();
        let mut roots: Vec<String> = vec![];
        for (name, td) in &ast_map {
            let parent = td.parent.as_ref().map(|p| p.name().to_string());
            match parent {
                Some(p) if ast_map.contains_key(&p) => {
                    children.entry(p).or_default().push(name.clone());
                }
                _ => roots.push(name.clone()),
            }
        }
        // Ordenar para asignación determinista de tags
        roots.sort();
        for kids in children.values_mut() { kids.sort(); }

        // DFS pre-order: cada tipo recibe tag antes que sus hijos → rango contiguo
        let mut tag_counter = 1u32;
        let mut tag_ranges: HashMap<String, (u32, u32)> = HashMap::new();
        for root in &roots {
            dfs_assign_tags(root, &children, &mut tag_counter, &mut tag_ranges);
        }

        // ── Pase 2: rellenar cuerpos y registrar layouts ──────────────────────
        for decl in decls {
            let td = match decl { Decl::Type(t) => t, _ => continue };

            // Campos: padre primero (recursivo), luego propios en orden del AST
            let field_names = collect_field_names(&td.name, &ast_map);

            // Body: [i32 type_tag, ptr vtable_ptr, campo0, campo1, ...]
            let mut body: Vec<BasicTypeEnum<'ctx>> = vec![
                self.context.i32_type().into(),
                self.ptr_type().into(),
            ];
            for fname in &field_names {
                let hulk_ty = self.find_field_type(&td.name, fname);
                body.push(self.hulk_type_to_llvm(&hulk_ty));
            }
            struct_map[&td.name].set_body(&body, false);

            // Métodos: padre primero, override = mismo slot (no se duplica)
            let method_names = collect_method_names(&td.name, &ast_map);
            let vt_body = vec![self.ptr_type().into(); method_names.len()];
            vtable_map[&td.name].set_body(&vt_body, false);

            // Vtable global sin inicializar — se rellena en lower_decl después
            // de emitir los métodos (necesitamos sus FunctionValues para los fn ptrs)
            let vtable_global = self.module.add_global(
                vtable_map[&td.name],
                Some(AddressSpace::default()),
                &format!("vtable_{}", td.name),
            );

            // Tags DFS calculados en Pase 1b
            let (type_tag, max_tag) = tag_ranges
                .get(&td.name)
                .copied()
                .unwrap_or_else(|| {
                    let t = self.type_registry.alloc_tag();
                    (t, t)
                });

            self.type_registry.layouts.insert(td.name.clone(), TypeLayout {
                struct_type:   struct_map[&td.name],
                vtable_type:   vtable_map[&td.name],
                vtable_global,
                field_names,
                method_names,
                type_tag,
                max_tag,
                ctor_fn:       None,
                parent:        td.parent.as_ref().map(|p| p.name().to_string()),
            });
        }

        Ok(())
    }
}

// ── Helpers libres ────────────────────────────────────────────────────────────

/// Campos en orden: padre primero (recursivo), luego propios.
/// Fuente de orden: TypeDecl.members (Vec), nunca TypeInfo.attributes (HashMap).
pub fn collect_field_names(
    type_name: &str,
    ast_map:   &HashMap<String, &TypeDecl>,
) -> Vec<String> {
    let td = match ast_map.get(type_name) { Some(t) => t, None => return vec![] };
    let mut names = match &td.parent {
        Some(p) => collect_field_names(p.name(), ast_map),
        None    => vec![],
    };
    for member in &td.members {
        if let TypeMember::Attribute(attr) = member {
            if !names.contains(&attr.name) {
                names.push(attr.name.clone());
            }
        }
    }
    names
}

/// Asigna tags en orden DFS pre-order para que todos los subtipos de un tipo
/// queden en el rango contiguo [type_tag, max_tag]. Permite range check en `is`.
fn dfs_assign_tags(
    name:     &str,
    children: &HashMap<String, Vec<String>>,
    counter:  &mut u32,
    ranges:   &mut HashMap<String, (u32, u32)>,
) {
    let min_tag = *counter;
    *counter += 1;
    if let Some(kids) = children.get(name) {
        for kid in kids {
            dfs_assign_tags(kid, children, counter, ranges);
        }
    }
    ranges.insert(name.to_string(), (min_tag, *counter - 1));
}

/// Slots de vtable: padre primero, nuevos métodos al final.
/// Override = el nombre ya está → no se añade (mismo slot).
pub fn collect_method_names(
    type_name: &str,
    ast_map:   &HashMap<String, &TypeDecl>,
) -> Vec<String> {
    let td = match ast_map.get(type_name) { Some(t) => t, None => return vec![] };
    let mut names = match &td.parent {
        Some(p) => collect_method_names(p.name(), ast_map),
        None    => vec![],
    };
    for member in &td.members {
        if let TypeMember::Method(m) = member {
            if !names.contains(&m.name) {
                names.push(m.name.clone());
            }
        }
    }
    names
}

