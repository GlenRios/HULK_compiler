use std::collections::HashMap;

use inkwell::AddressSpace;
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::{BasicTypeEnum, FloatType, FunctionType, IntType, PointerType};
use inkwell::values::{BasicValue, BasicValueEnum, BasicMetadataValueEnum, FloatValue, FunctionValue, IntValue, PointerValue};
use inkwell::FloatPredicate;

use crate::semantic::{HulkType, TypeHierarchy, SemanticOutput};
use crate::semantic::type_system::FuncSignature;
use super::error::{CodegenError, CodegenResult};
use super::objects::ObjectRegistry;
use super::symbols::{SymbolTable, Place};
use super::value::CgValue;

pub struct CodegenContext<'ctx> {
    pub context:           &'ctx Context,
    pub module:            Module<'ctx>,
    pub builder:           Builder<'ctx>,
    pub symbols:           SymbolTable<'ctx>,
    pub functions:         HashMap<String, FunctionValue<'ctx>>,
    pub current_function:  Option<FunctionValue<'ctx>>,
    // ── Fase 5: soporte para tipos de usuario ──
    pub type_hierarchy:    TypeHierarchy,
    pub type_registry:     ObjectRegistry<'ctx>,
    pub self_ptr:            Option<PointerValue<'ctx>>,
    pub current_type_name:   Option<String>,
    /// Nombre del método que se está compilando ahora mismo.
    /// Necesario para resolver base() — permite saber qué método del padre llamar.
    pub current_method_name: Option<String>,
    /// node_id → tipo de cada expresión; producido por el TypeChecker, consumido en lower_expr.
    pub expr_types:        HashMap<u32, HulkType>,
}

impl<'ctx> CodegenContext<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        Self {
            context,
            module:            context.create_module(module_name),
            builder:           context.create_builder(),
            symbols:           SymbolTable::new(),
            functions:         HashMap::new(),
            current_function:  None,
            type_hierarchy:    TypeHierarchy::new(),
            type_registry:     ObjectRegistry::new(),
            self_ptr:            None,
            current_type_name:   None,
            current_method_name: None,
            expr_types:          HashMap::new(),
        }
    }

    /// Construye el contexto a partir del output completo del análisis semántico.
    pub fn from_semantic_output(
        context:     &'ctx Context,
        module_name: &str,
        output:      crate::semantic::SemanticOutput,
    ) -> Self {
        Self {
            context,
            module:            context.create_module(module_name),
            builder:           context.create_builder(),
            symbols:           SymbolTable::new(),
            functions:         HashMap::new(),
            current_function:  None,
            type_hierarchy:    output.hierarchy,
            type_registry:     ObjectRegistry::new(),
            self_ptr:            None,
            current_type_name:   None,
            current_method_name: None,
            expr_types:          output.expr_types,
        }
    }

    // ── Tipos LLVM básicos ────────────────────────────────────────────────────

    pub fn f64_type(&self) -> FloatType<'ctx> {
        self.context.f64_type()
    }

    pub fn bool_type(&self) -> IntType<'ctx> {
        self.context.bool_type()
    }

    pub fn ptr_type(&self) -> PointerType<'ctx> {
        self.context.i8_type().ptr_type(AddressSpace::default())
    }

    /// Traduce HulkType al BasicTypeEnum LLVM correspondiente.
    /// Usado al construir bodies de structs y firmas de funciones.
    pub fn hulk_type_to_llvm(&self, ty: &HulkType) -> BasicTypeEnum<'ctx> {
        match ty {
            HulkType::Number  => self.f64_type().into(),
            HulkType::Boolean => self.bool_type().into(),
            _                 => self.ptr_type().into(),
        }
    }

    /// Busca el HulkType de un campo subiendo la cadena de herencia.
    /// TypeInfo.attributes solo guarda los campos PROPIOS; para campos
    /// heredados hay que subir al padre (igual que lookup_attribute del TypeChecker).
    pub fn find_field_type(&self, type_name: &str, field_name: &str) -> HulkType {
        let mut cur = type_name.to_string();
        loop {
            if let Some(info) = self.type_hierarchy.types.get(&cur) {
                if let Some(ty) = info.attributes.get(field_name) {
                    return ty.clone();
                }
                match &info.parent {
                    Some(p) => cur = p.clone(),
                    None    => break,
                }
            } else { break; }
        }
        HulkType::Object
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

    pub fn create_entry_alloca_for(
        &self,
        function: FunctionValue<'ctx>,
        name:     &str,
        hulk_ty:  &HulkType,
    ) -> CodegenResult<Place<'ctx>> {
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

        Ok(Place { ptr, hulk_ty: hulk_ty.clone() })
    }

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

    /// Carga el valor almacenado en un Place.
    /// Usa hulk_ty para elegir el tipo LLVM correcto (f64, i1, ptr...).
    pub fn load_place(&self, place: &Place<'ctx>, name: &str) -> CodegenResult<CgValue<'ctx>> {
        match &place.hulk_ty {
            HulkType::Number => {
                let v = self.builder
                    .build_load(self.f64_type(), place.ptr, name)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .into_float_value();
                Ok(CgValue::Number(v))
            }
            HulkType::Boolean => {
                let v = self.builder
                    .build_load(self.bool_type(), place.ptr, name)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .into_int_value();
                Ok(CgValue::Bool(v))
            }
            HulkType::StringT => {
                let v = self.builder
                    .build_load(self.ptr_type(), place.ptr, name)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .into_pointer_value();
                Ok(CgValue::Str(v))
            }
            HulkType::Null => Ok(CgValue::Null),
            _ => {
                let v = self.builder
                    .build_load(self.ptr_type(), place.ptr, name)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .into_pointer_value();
                Ok(CgValue::Object(v))
            }
        }
    }

    /// Escribe un valor en un Place.
    pub fn store_place(&self, place: &Place<'ctx>, val: CgValue<'ctx>) -> CodegenResult<()> {
        match val {
            CgValue::Number(v) => {
                self.builder.build_store(place.ptr, v)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
            }
            CgValue::Bool(v) => {
                self.builder.build_store(place.ptr, v)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
            }
            CgValue::Str(v) | CgValue::Object(v) | CgValue::Vector(v) => {
                self.builder.build_store(place.ptr, v)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
            }
            CgValue::Null => {
                let null = self.ptr_type().const_null();
                self.builder.build_store(place.ptr, null)
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
            }
            CgValue::Void => {}
        }
        Ok(())
    }

    /// Calcula el Place de un campo de un objeto: hace el GEP y devuelve (ptr, tipo).
    /// Centraliza toda la lógica de acceso a campos — usada en lectura, escritura e inicialización.
    pub fn field_place(
        &self,
        obj_ptr:    PointerValue<'ctx>,
        type_name:  &str,
        field_name: &str,
    ) -> CodegenResult<Place<'ctx>> {
        let layout = self.type_registry.layouts.get(type_name)
            .ok_or_else(|| CodegenError::Unsupported(
                format!("tipo '{}' no registrado en ObjectRegistry", type_name)))?;

        let field_idx = self.type_registry
            .field_llvm_index(type_name, field_name)
            .ok_or_else(|| CodegenError::Unsupported(
                format!("campo '{}' no encontrado en '{}'", field_name, type_name)))?;

        let ptr = self.builder
            .build_struct_gep(layout.struct_type, obj_ptr, field_idx, field_name)
            .map_err(|e| CodegenError::Builder(e.to_string()))?;

        let hulk_ty = self.find_field_type(type_name, field_name);

        Ok(Place { ptr, hulk_ty })
    }

    /// Resuelve el despacho virtual de un método y devuelve todo lo necesario para la llamada:
    ///   - fn_ptr   → puntero a función cargado de la vtable del objeto en runtime
    ///   - fn_type  → firma LLVM (para build_indirect_call)
    ///   - sig      → firma semántica (para conocer tipos de parámetros y retorno)
    ///
    /// Usa el tipo estático para encontrar el slot (invariante: mismo slot en padre e hijo)
    /// pero carga la vtable del objeto real en runtime (dispatch dinámico correcto).
    pub fn method_dispatch(
        &self,
        obj_ptr:     PointerValue<'ctx>,
        type_name:   &str,
        method_name: &str,
    ) -> CodegenResult<(PointerValue<'ctx>, FunctionType<'ctx>, FuncSignature)> {
        let layout = self.type_registry.layouts.get(type_name)
            .ok_or_else(|| CodegenError::Unsupported(
                format!("tipo '{}' no registrado en ObjectRegistry", type_name)))?;

        // Cargar vtable_ptr desde obj[1] usando el tipo estático para el GEP
        // (el vtable_ptr siempre está en campo 1 en padre e hijo — misma posición)
        let vtable_field = self.builder
            .build_struct_gep(layout.struct_type, obj_ptr, 1, "vtable_field")
            .map_err(|e| CodegenError::Builder(e.to_string()))?;
        let vtable_ptr = self.builder
            .build_load(self.ptr_type(), vtable_field, "vtable_ptr")
            .map_err(|e| CodegenError::Builder(e.to_string()))?
            .into_pointer_value();

        // Slot del método en la vtable del tipo estático
        // (mismo slot en subtipos gracias a collect_method_names)
        let slot = self.type_registry
            .method_slot(type_name, method_name)
            .ok_or_else(|| CodegenError::Unsupported(
                format!("método '{}' no encontrado en vtable de '{}'", method_name, type_name)))?;

        // Cargar el puntero a función desde la vtable del objeto real (dispatch dinámico)
        let fn_ptr_field = self.builder
            .build_struct_gep(layout.vtable_type, vtable_ptr, slot, "fn_ptr_field")
            .map_err(|e| CodegenError::Builder(e.to_string()))?;
        let fn_ptr = self.builder
            .build_load(self.ptr_type(), fn_ptr_field, "fn_ptr")
            .map_err(|e| CodegenError::Builder(e.to_string()))?
            .into_pointer_value();

        // Firma LLVM — desde la función estática, solo para conocer los tipos
        // (la función real la decide la vtable en runtime)
        let impl_type = self.type_hierarchy
            .find_method_impl_type(type_name, method_name)
            .unwrap_or_else(|| type_name.to_string());
        let static_fn_name = format!("__hulk_method_{}_{}", impl_type, method_name);
        let fn_type = self.module
            .get_function(&static_fn_name)
            .ok_or_else(|| CodegenError::UnknownFunction(static_fn_name.clone()))?
            .get_type();

        // Firma semántica (tipos de parámetros y tipo de retorno)
        let sig = self.type_hierarchy.types
            .get(&impl_type)
            .and_then(|ti| ti.methods.get(method_name))
            .cloned()
            .ok_or_else(|| CodegenError::Unsupported(
                format!("firma de '{}' no encontrada en TypeHierarchy", method_name)))?;

        Ok((fn_ptr, fn_type, sig))
    }

    /// Convierte un CgValue al tipo LLVM correcto para pasarlo como argumento
    /// según el HulkType que espera la función receptora.
    /// Centraliza la lógica de coerción que se repite en constructores y method calls.
    pub fn coerce_arg(
        &self,
        val:      CgValue<'ctx>,
        expected: &HulkType,
    ) -> CodegenResult<inkwell::values::BasicMetadataValueEnum<'ctx>> {
        match expected {
            HulkType::Number  => Ok(self.require_number(val)?.into()),
            HulkType::Boolean => Ok(self.require_bool(val)?.into()),
            _ => match val {
                CgValue::Object(p) | CgValue::Str(p) | CgValue::Vector(p) => Ok(p.into()),
                CgValue::Null => Ok(self.ptr_type().const_null().into()),
                other => Ok(self.require_number(other)?.into()),
            }
        }
    }

    // ── Coerciones ────────────────────────────────────────────────────────────

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
                let true_str  = self.builder.build_global_string_ptr("true",  "true_s")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?.as_pointer_value();
                let false_str = self.builder.build_global_string_ptr("false", "false_s")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?.as_pointer_value();
                Ok(self.builder
                    .build_select(b, true_str, false_str, "bool_str")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .into_pointer_value())
            }
            CgValue::Str(p) => Ok(p),
            CgValue::Null   => {
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
            CgValue::Void      => Err(CodegenError::Unsupported("void en contexto numerico".to_string())),
            _ => Err(CodegenError::Unsupported("tipo no numerico en contexto numerico".to_string())),
        }
    }

    pub fn require_bool(&self, value: CgValue<'ctx>) -> CodegenResult<IntValue<'ctx>> {
        match value {
            CgValue::Bool(v)   => Ok(v),
            CgValue::Number(v) => self.builder
                .build_float_compare(FloatPredicate::ONE, v, self.f64_type().const_float(0.0), "num_to_bool")
                .map_err(|e| CodegenError::Builder(e.to_string())),
            CgValue::Null      => Ok(self.bool_type().const_int(0, false)),
            CgValue::Void      => Err(CodegenError::Unsupported("void en contexto booleano".to_string())),
            _ => Err(CodegenError::Unsupported("tipo no booleano en contexto booleano".to_string())),
        }
    }

    /// Dispatch dinámico para receptores con tipo protocolo.
    ///
    /// Carga el type_tag del objeto en runtime y genera un switch de LLVM
    /// con un case por cada tipo concreto que conforma el protocolo.
    /// Cada case llama directamente a la implementación concreta del método.
    /// Los resultados se mergean con un phi node.
    pub fn method_dispatch_protocol(
        &mut self,
        obj_ptr:     PointerValue<'ctx>,
        proto_name:  &str,
        method_name: &str,
        user_args:   &[CgValue<'ctx>],
        return_ty:   &HulkType,
    ) -> CodegenResult<CgValue<'ctx>> {
        // 1. Tipos que conforman el protocolo y tienen layout LLVM registrado
        let all_names: Vec<String> = self.type_registry.layouts.keys().cloned().collect();
        let conforming: Vec<(String, u32)> = all_names.iter()
            .filter(|name| self.type_hierarchy.conforms_protocol(name, proto_name))
            .map(|name| (name.clone(), self.type_registry.layouts[name].type_tag))
            .collect();

        // 2. Firma de parámetros del primer tipo conformante — todos son compatibles
        //    (el semantic checker garantizó eso al verificar conformancia)
        let param_sig: Option<FuncSignature> = conforming.first()
            .and_then(|(name, _)| {
                let mut cur = name.clone();
                loop {
                    if let Some(ti) = self.type_hierarchy.types.get(&cur) {
                        if let Some(sig) = ti.methods.get(method_name) {
                            return Some(sig.clone());
                        }
                        match &ti.parent {
                            Some(p) => cur = p.clone(),
                            None    => return None,
                        }
                    } else {
                        return None;
                    }
                }
            });

        // 3. Cargar type_tag desde obj_ptr[0] — igual que en el operador is
        let i32_ty = self.context.i32_type();
        let runtime_tag = self.builder
            .build_load(i32_ty, obj_ptr, "proto_tag")
            .map_err(|e| CodegenError::Builder(e.to_string()))?
            .into_int_value();

        // 4. Crear bloques: uno por tipo conformante + default (unreachable) + merge
        let cur_fn = self.current_function
            .ok_or_else(|| CodegenError::Unsupported(
                "method_dispatch_protocol sin función activa".into()))?;
        let default_bb = self.context.append_basic_block(cur_fn, "proto_unreachable");
        let merge_bb   = self.context.append_basic_block(cur_fn, "proto_merge");

        let case_blocks: Vec<(IntValue<'ctx>, BasicBlock<'ctx>, String)> =
            conforming.iter().map(|(name, tag)| {
                let bb  = self.context.append_basic_block(cur_fn, &format!("proto_case_{}", name));
                let val = i32_ty.const_int(*tag as u64, false);
                (val, bb, name.clone())
            }).collect();

        // 5. Emitir switch — termina el bloque actual (es un terminator de LLVM)
        let arms: Vec<(IntValue<'ctx>, BasicBlock<'ctx>)> =
            case_blocks.iter().map(|(v, bb, _)| (*v, *bb)).collect();
        self.builder
            .build_switch(runtime_tag, default_bb, &arms)
            .map_err(|e| CodegenError::Builder(e.to_string()))?;

        // 6. Default → unreachable (conformancia garantizada por el semantic checker)
        self.builder.position_at_end(default_bb);
        self.builder.build_unreachable()
            .map_err(|e| CodegenError::Builder(e.to_string()))?;

        // 7. Llenar cada case: llamada directa al método concreto + salto a merge
        let mut phi_incoming: Vec<(BasicValueEnum<'ctx>, BasicBlock<'ctx>)> = vec![];

        for (_, case_bb, type_name) in &case_blocks {
            self.builder.position_at_end(*case_bb);

            let mut call_args: Vec<BasicMetadataValueEnum<'ctx>> = vec![obj_ptr.into()];
            for (i, arg_val) in user_args.iter().enumerate() {
                let expected = param_sig.as_ref()
                    .and_then(|s| s.params.get(i))
                    .map(|(_, t)| t.clone())
                    .unwrap_or(HulkType::Object);
                call_args.push(self.coerce_arg(arg_val.clone(), &expected)?);
            }

            // Llamada directa (no indirecta) a la implementación concreta del tipo
            let fn_name = format!("__hulk_method_{}_{}", type_name, method_name);
            let fn_val  = self.module.get_function(&fn_name)
                .ok_or_else(|| CodegenError::Unsupported(
                    format!("función '{}' no encontrada en módulo", fn_name)))?;

            let call   = self.builder
                .build_call(fn_val, &call_args, "proto_call")
                .map_err(|e| CodegenError::Builder(e.to_string()))?;
            let result = call.try_as_basic_value().left()
                .ok_or_else(|| CodegenError::Unsupported(
                    "método de protocolo sin valor de retorno".into()))?;

            phi_incoming.push((result, *case_bb));
            self.builder
                .build_unconditional_branch(merge_bb)
                .map_err(|e| CodegenError::Builder(e.to_string()))?;
        }

        // 8. Phi node en merge — el tipo viene de return_ty (anotado por el semantic checker)
        self.builder.position_at_end(merge_bb);
        let llvm_ty = self.hulk_type_to_llvm(return_ty);
        let phi = self.builder
            .build_phi(llvm_ty, "proto_ret")
            .map_err(|e| CodegenError::Builder(e.to_string()))?;
        for (val, bb) in &phi_incoming {
            phi.add_incoming(&[(&*val as &dyn BasicValue<'ctx>, *bb)]);
        }

        let bv = phi.as_basic_value();
        Ok(match return_ty {
            HulkType::Number  => CgValue::Number(bv.into_float_value()),
            HulkType::Boolean => CgValue::Bool(bv.into_int_value()),
            HulkType::StringT => CgValue::Str(bv.into_pointer_value()),
            _                 => CgValue::Object(bv.into_pointer_value()),
        })
    }
}
