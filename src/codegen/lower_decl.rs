use inkwell::types::BasicType;
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue, PointerValue};

use crate::parser::ast::{Decl, FuncDecl, MethodDef, TypeDecl, TypeMember};
use crate::semantic::HulkType;

use super::context::CodegenContext;
use super::error::{CodegenError, CodegenResult};
use super::value::CgValue;
use super::visitor::{DeclVisitor, ExprVisitor};

impl<'ctx> CodegenContext<'ctx> {
    fn lower_type_decl(&mut self, td: &TypeDecl) -> CodegenResult<()> {
        // 1. Emitir métodos propios del tipo
        for member in &td.members {
            if let TypeMember::Method(method) = member {
                self.lower_method(&td.name, method)?;
            }
        }

        // 2. Emitir constructor
        let ctor = self.lower_constructor(td)?;
        if let Some(layout) = self.type_registry.layouts.get_mut(&td.name) {
            layout.ctor_fn = Some(ctor);
        }

        // 3. Inicializar vtable global — debe ir después de emitir métodos
        // porque init_vtable_global busca las funciones por nombre en el módulo
        self.init_vtable_global(&td.name)?;

        Ok(())
    }

    fn lower_method(
        &mut self,
        type_name: &str,
        method:    &MethodDef,
    ) -> CodegenResult<FunctionValue<'ctx>> {
        let fn_name = format!("__hulk_method_{}_{}", type_name, method.name);

        // Tipo de retorno del método desde TypeHierarchy
        let ret_hulk_ty = self.type_hierarchy.types
            .get(type_name)
            .and_then(|ti| ti.methods.get(&method.name))
            .map(|sig| sig.return_type.clone())
            .unwrap_or(HulkType::Number);

        // Firma: (ptr self, param1, param2, ...) -> ret_type
        let mut param_types: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> =
            vec![self.ptr_type().into()];

        for param in &method.params {
            let hulk_ty = self.type_hierarchy.types
                .get(type_name)
                .and_then(|ti| ti.methods.get(&method.name))
                .and_then(|sig| sig.params.iter().find(|(n, _)| n == &param.name))
                .map(|(_, ty)| ty.clone())
                .unwrap_or(HulkType::Number);
            param_types.push(self.hulk_type_to_llvm(&hulk_ty).into());
        }

        let ret_llvm_ty = self.hulk_type_to_llvm(&ret_hulk_ty);
        let fn_type = ret_llvm_ty.fn_type(&param_types, false);
        let function = self.module.add_function(&fn_name, fn_type, None);

        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        self.current_function  = Some(function);
        self.current_type_name   = Some(type_name.to_string());
        self.current_method_name = Some(method.name.clone());
        self.push_scope();

        // Parámetro 0 = self → alloca + store + meter en symbol table
        let self_param = function.get_nth_param(0).unwrap().into_pointer_value();
        self.self_ptr = Some(self_param);
        let self_slot = self.create_entry_alloca_for(
            function, "self", &HulkType::UserDefined(type_name.to_string()),
        )?;
        self.builder.build_store(self_slot.ptr, self_param)
            .map_err(|e| CodegenError::Builder(e.to_string()))?;
        self.symbols.insert("self".to_string(), self_slot);

        // Parámetros de usuario (índices 1, 2, ...)
        for (idx, param) in method.params.iter().enumerate() {
            let pval = function.get_nth_param(1 + idx as u32).unwrap();
            let hulk_ty = self.type_hierarchy.types
                .get(type_name)
                .and_then(|ti| ti.methods.get(&method.name))
                .and_then(|sig| sig.params.iter().find(|(n, _)| n == &param.name))
                .map(|(_, ty)| ty.clone())
                .unwrap_or(HulkType::Number);
            let slot = self.create_entry_alloca_for(function, &param.name, &hulk_ty)?;
            let store_val: BasicValueEnum = match &hulk_ty {
                HulkType::Number  => pval.into_float_value().into(),
                HulkType::Boolean => pval.into_int_value().into(),
                _                 => pval.into_pointer_value().into(),
            };
            self.builder.build_store(slot.ptr, store_val)
                .map_err(|e| CodegenError::Builder(e.to_string()))?;
            self.symbols.insert(param.name.clone(), slot);
        }

        // Compilar cuerpo
        let body_val = self.visit_expr(&method.body)?;
        self.emit_typed_return(body_val, &ret_hulk_ty)?;

        self.pop_scope();
        self.self_ptr            = None;
        self.current_type_name   = None;
        self.current_method_name = None;
        self.current_function    = None;
        Ok(function)
    }

    fn lower_constructor(&mut self, td: &TypeDecl) -> CodegenResult<FunctionValue<'ctx>> {
        let fn_name = format!("__hulk_ctor_{}", td.name);

        // Parámetros del constructor desde TypeHierarchy (ya con tipos resueltos)
        let ctor_params: Vec<(String, HulkType)> = self.type_hierarchy.types
            .get(&td.name)
            .map(|ti| ti.constructor_params.clone())
            .unwrap_or_default();

        // Firma: (param0_type, param1_type, ...) -> ptr
        let param_types: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> = ctor_params.iter()
            .map(|(_, ty)| self.hulk_type_to_llvm(ty).into())
            .collect();
        let fn_type = self.ptr_type().fn_type(&param_types, false);
        let function = self.module.add_function(&fn_name, fn_type, None);

        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        self.current_function = Some(function);
        self.push_scope();

        // ── 1. malloc(sizeof(%TypeName)) ─────────────────────────────────────
        let struct_ty = self.type_registry.layouts[&td.name].struct_type;
        let size = struct_ty.size_of()
            .ok_or_else(|| CodegenError::Unsupported(
                format!("tipo '{}' sin tamaño — struct opaco?", td.name)))?;
        let malloc_fn = self.require_fn("malloc")?;
        let raw = self.builder
            .build_call(malloc_fn, &[size.into()], "raw")
            .map_err(|e| CodegenError::Builder(e.to_string()))?
            .try_as_basic_value().left().unwrap().into_pointer_value();

        // ── 2. Escribir type_tag en campo 0 ──────────────────────────────────
        let tag = self.type_registry.layouts[&td.name].type_tag;
        let tag_ptr = self.builder
            .build_struct_gep(struct_ty, raw, 0, "tag_ptr")
            .map_err(|e| CodegenError::Builder(e.to_string()))?;
        self.builder
            .build_store(tag_ptr, self.context.i32_type().const_int(tag as u64, false))
            .map_err(|e| CodegenError::Builder(e.to_string()))?;

        // ── 3. Escribir vtable ptr en campo 1 ────────────────────────────────
        let vtable_ptr = self.type_registry.layouts[&td.name]
            .vtable_global.as_pointer_value();
        let vt_field = self.builder
            .build_struct_gep(struct_ty, raw, 1, "vt_field")
            .map_err(|e| CodegenError::Builder(e.to_string()))?;
        self.builder
            .build_store(vt_field, vtable_ptr)
            .map_err(|e| CodegenError::Builder(e.to_string()))?;

        // ── 4. Meter params del ctor en la symbol table ───────────────────────
        // Necesario porque los inicializadores de atributos los referencian:
        //   type Point(x) { x = x; }  ← el "x" del lado derecho es el param
        for (idx, (pname, ptype)) in ctor_params.iter().enumerate() {
            let pval = function.get_nth_param(idx as u32).unwrap();
            let slot = self.create_entry_alloca_for(function, pname, ptype)?;
            let store_val: BasicValueEnum = match ptype {
                HulkType::Number  => pval.into_float_value().into(),
                HulkType::Boolean => pval.into_int_value().into(),
                _                 => pval.into_pointer_value().into(),
            };
            self.builder.build_store(slot.ptr, store_val)
                .map_err(|e| CodegenError::Builder(e.to_string()))?;
            self.symbols.insert(pname.clone(), slot);
        }

        // ── 4.5. Inicializar atributos heredados del padre ───────────────────
        if let Some(parent_type) = &td.parent {
            let parent_name = parent_type.name().to_string();
            let parent_args = td.parent_args.clone();
            let type_name   = td.name.clone();
            self.init_inherited_fields(raw, &type_name, &parent_name, parent_args, function)?;
        }

        // ── 5. Compilar inicializadores y escribir en campos ──────────────────
        for member in &td.members {
            if let TypeMember::Attribute(attr) = member {
                let val   = self.visit_expr(&attr.value)?;
                let place = self.field_place(raw, &td.name, &attr.name)?;
                self.store_place(&place, val)?;
            }
        }

        // ── 6. Retornar puntero al objeto ─────────────────────────────────────
        self.builder.build_return(Some(&raw as &dyn BasicValue))
            .map_err(|e| CodegenError::Builder(e.to_string()))?;

        self.pop_scope();
        self.current_function = None;
        Ok(function)
    }

    fn init_vtable_global(&self, type_name: &str) -> CodegenResult<()> {
        let layout = &self.type_registry.layouts[type_name];

        // Recoger el fn ptr de cada slot del vtable en orden
        let mut fn_ptrs: Vec<inkwell::values::BasicValueEnum<'ctx>> = vec![];
        for method_name in &layout.method_names {
            // Quién implementa este método: el propio tipo o un ancestro
            let impl_type = self.type_hierarchy
                .find_method_impl_type(type_name, method_name)
                .unwrap_or_else(|| type_name.to_string());

            let fn_name = format!("__hulk_method_{}_{}", impl_type, method_name);
            let fn_val = self.module.get_function(&fn_name)
                .ok_or_else(|| CodegenError::UnknownFunction(fn_name.clone()))?;

            fn_ptrs.push(fn_val.as_global_value().as_pointer_value().into());
        }

        // Crear la constante struct e inicializar el global
        let vtable_const = layout.vtable_type.const_named_struct(&fn_ptrs);
        layout.vtable_global.set_initializer(&vtable_const);
        layout.vtable_global.set_constant(true);
        Ok(())
    }

    /// Inicializa en `raw` (objeto del tipo `child_type_name`) los campos que pertenecen
    /// a `parent_type_name` y sus ancestros, usando `parent_args` como argumentos al
    /// constructor del padre.
    ///
    /// Se llama desde `lower_constructor` cuando el tipo a construir hereda de otro:
    ///   type Child(v) inherits Base(v) { }
    /// → evalúa `v` (parent_args) en el scope del ctor de Child, luego ejecuta los
    ///   inicializadores de Base colocando los resultados en los campos de `raw`.
    fn init_inherited_fields(
        &mut self,
        raw:              PointerValue<'ctx>,
        child_type_name:  &str,
        parent_type_name: &str,
        parent_args:      Vec<crate::parser::ast::Expr>,
        function:         FunctionValue<'ctx>,
    ) -> CodegenResult<()> {
        // Obtener el TypeDecl del padre (clonado para evitar conflictos de borrow)
        let parent_td = self.type_decls.get(parent_type_name).cloned()
            .ok_or_else(|| CodegenError::Unsupported(
                format!("TypeDecl de '{}' no encontrado para inicializar herencia", parent_type_name)))?;

        // Parámetros del constructor del padre (nombre + tipo)
        let parent_ctor_params: Vec<(String, HulkType)> = self.type_hierarchy.types
            .get(parent_type_name)
            .map(|ti| ti.constructor_params.clone())
            .unwrap_or_default();

        // Evaluar parent_args en el scope actual (el del constructor del hijo)
        let mut arg_vals: Vec<super::value::CgValue<'ctx>> = Vec::new();
        for expr in &parent_args {
            arg_vals.push(self.visit_expr(expr)?);
        }

        // Abrir un nuevo scope y ligar los nombres de params del padre a los valores evaluados
        self.push_scope();
        for ((pname, ptype), val) in parent_ctor_params.iter().zip(arg_vals.iter()) {
            let slot = self.create_entry_alloca_for(function, pname, ptype)?;
            let store_val: BasicValueEnum = match ptype {
                HulkType::Number  => self.require_number(*val)?.into(),
                HulkType::Boolean => self.require_bool(*val)?.into(),
                _ => match val {
                    super::value::CgValue::Object(p) |
                    super::value::CgValue::Str(p)    |
                    super::value::CgValue::Vector(p) => (*p).into(),
                    super::value::CgValue::Null      => self.ptr_type().const_null().into(),
                    _ => return Err(CodegenError::Unsupported(
                        format!("tipo inesperado al pasar arg heredado para '{}'", pname))),
                }
            };
            self.builder.build_store(slot.ptr, store_val)
                .map_err(|e| CodegenError::Builder(e.to_string()))?;
            self.symbols.insert(pname.clone(), slot);
        }

        // Primero inicializar los campos del abuelo (recursión)
        if let Some(grandparent_type) = &parent_td.parent {
            let gp_name = grandparent_type.name().to_string();
            let gp_args = parent_td.parent_args.clone();
            self.init_inherited_fields(raw, child_type_name, &gp_name, gp_args, function)?;
        }

        // Luego los campos propios del padre
        for member in &parent_td.members {
            if let TypeMember::Attribute(attr) = member {
                let val   = self.visit_expr(&attr.value)?;
                let place = self.field_place(raw, child_type_name, &attr.name)?;
                self.store_place(&place, val)?;
            }
        }

        self.pop_scope();
        Ok(())
    }

    pub fn predeclare_functions(&mut self, decls: &[Decl]) {
        for decl in decls {
            if let Decl::Function(func) = decl {
                let sig = self.func_sigs.get(&func.name).cloned();

                let param_types: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> =
                    if let Some(ref s) = sig {
                        s.params.iter()
                            .map(|(_, ty)| self.hulk_type_to_llvm(ty).into())
                            .collect()
                    } else {
                        vec![self.f64_type().into(); func.params.len()]
                    };

                let ret_ty = sig.as_ref()
                    .map(|s| s.return_type.clone())
                    .unwrap_or(HulkType::Number);

                let fn_type = self.hulk_type_to_llvm(&ret_ty).fn_type(&param_types, false);
                let function = self.module.add_function(&func.name, fn_type, None);
                self.functions.insert(func.name.clone(), function);
            }
        }
    }

    fn lower_function_decl(&mut self, func_decl: &FuncDecl) -> CodegenResult<()> {
        let function = self
            .functions
            .get(&func_decl.name)
            .copied()
            .ok_or_else(|| CodegenError::UnknownFunction(func_decl.name.clone()))?;

        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        self.current_function = Some(function);
        self.push_scope();

        let sig = self.func_sigs.get(&func_decl.name).cloned();

        for (idx, param_decl) in func_decl.params.iter().enumerate() {
            if let Some(pval) = function.get_nth_param(idx as u32) {
                let hulk_ty = sig.as_ref()
                    .and_then(|s| s.params.get(idx))
                    .map(|(_, ty)| ty.clone())
                    .unwrap_or(HulkType::Number);

                let slot = self.create_entry_alloca_for(function, &param_decl.name, &hulk_ty)?;
                let store_val: BasicValueEnum = match &hulk_ty {
                    HulkType::Number  => pval.into_float_value().into(),
                    HulkType::Boolean => pval.into_int_value().into(),
                    _                 => pval.into_pointer_value().into(),
                };
                self.builder
                    .build_store(slot.ptr, store_val)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                self.symbols.insert(param_decl.name.clone(), slot);
            }
        }

        let body_value = self.visit_expr(&func_decl.body)?;
        let ret_ty = sig.map(|s| s.return_type).unwrap_or(HulkType::Number);
        self.emit_typed_return(body_value, &ret_ty)?;

        self.pop_scope();
        self.current_function = None;
        Ok(())
    }
}

impl<'ctx> DeclVisitor<'ctx> for CodegenContext<'ctx> {
    fn visit_decl(&mut self, decl: &Decl) -> CodegenResult<()> {
        match decl {
            Decl::Function(func) => self.lower_function_decl(func),
            Decl::Type(td) => self.lower_type_decl(td),
            Decl::Protocol(_) => Ok(()),
        }
    }
}
