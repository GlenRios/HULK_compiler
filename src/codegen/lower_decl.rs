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
        if !self.is_current_block_terminated() {
            match &ret_hulk_ty {
                HulkType::Number => {
                    let v = self.require_number(body_val)?;
                    self.builder.build_return(Some(&v as &dyn BasicValue))
                        .map_err(|e| CodegenError::Builder(e.to_string()))?;
                }
                HulkType::Boolean => {
                    let v = self.require_bool(body_val)?;
                    self.builder.build_return(Some(&v as &dyn BasicValue))
                        .map_err(|e| CodegenError::Builder(e.to_string()))?;
                }
                HulkType::Null | HulkType::Unknown => {
                    self.builder.build_return(None)
                        .map_err(|e| CodegenError::Builder(e.to_string()))?;
                }
                _ => {
                    // String, Object, UserDefined → ptr
                    let ptr: PointerValue = match body_val {
                        CgValue::Str(p) | CgValue::Object(p) | CgValue::Vector(p) => p,
                        CgValue::Null => self.ptr_type().const_null(),
                        _ => self.ptr_type().const_null(),
                    };
                    self.builder.build_return(Some(&ptr as &dyn BasicValue))
                        .map_err(|e| CodegenError::Builder(e.to_string()))?;
                }
            }
        }

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
        let malloc_fn = self.require_fn("malloc");
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

    pub fn predeclare_functions(&mut self, decls: &[Decl]) {
        for decl in decls {
            if let Decl::Function(func) = decl {
                let param_types = vec![self.f64_type().into(); func.params.len()];
                let fn_type = self.f64_type().fn_type(&param_types, false);
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

        for (idx, param_decl) in func_decl.params.iter().enumerate() {
            if let Some(param) = function.get_nth_param(idx as u32) {
                let param_val = param.into_float_value();
                // Parámetros de función siguen siendo f64 por ahora (Fase 6 los generaliza)
                let slot = self.create_entry_alloca_for(
                    function,
                    &param_decl.name,
                    &crate::semantic::HulkType::Number,
                )?;
                self.builder
                    .build_store(slot.ptr, param_val)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
                self.symbols.insert(param_decl.name.clone(), slot);
            }
        }

        let body_value = self.visit_expr(&func_decl.body)?;

        if !self.is_current_block_terminated() {
            let ret = self.require_number(body_value)?;
            self.builder
                .build_return(Some(&ret))
                .map_err(|e| CodegenError::Builder(e.to_string()))?;
        }

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
            Decl::Protocol(_) => Err(CodegenError::Unsupported(
                "codegen de ProtocolDecl aun no implementado".to_string(),
            )),
        }
    }
}
