use inkwell::basic_block::BasicBlock;
use inkwell::types::FunctionType;
use inkwell::values::{BasicValue, BasicValueEnum, BasicMetadataValueEnum, IntValue, PointerValue};

use crate::semantic::{FuncSignature, HulkType};

use super::context::CodegenContext;
use super::error::{CodegenError, CodegenResult};
use super::symbols::Place;
use super::value::CgValue;

impl<'ctx> CodegenContext<'ctx> {
    /// Calcula el Place de un campo de un objeto (GEP + tipo).
    /// Usada en lectura, escritura e inicialización de atributos.
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

    /// Despacho virtual a través de vtable.
    /// Devuelve (fn_ptr, fn_type LLVM, firma semántica).
    pub fn method_dispatch(
        &self,
        obj_ptr:     PointerValue<'ctx>,
        type_name:   &str,
        method_name: &str,
    ) -> CodegenResult<(PointerValue<'ctx>, FunctionType<'ctx>, FuncSignature)> {
        let layout = self.type_registry.layouts.get(type_name)
            .ok_or_else(|| CodegenError::Unsupported(
                format!("tipo '{}' no registrado en ObjectRegistry", type_name)))?;

        let vtable_field = self.builder
            .build_struct_gep(layout.struct_type, obj_ptr, 1, "vtable_field")
            .map_err(|e| CodegenError::Builder(e.to_string()))?;
        let vtable_ptr = self.builder
            .build_load(self.ptr_type(), vtable_field, "vtable_ptr")
            .map_err(|e| CodegenError::Builder(e.to_string()))?
            .into_pointer_value();

        let slot = self.type_registry
            .method_slot(type_name, method_name)
            .ok_or_else(|| CodegenError::Unsupported(
                format!("método '{}' no encontrado en vtable de '{}'", method_name, type_name)))?;

        let fn_ptr_field = self.builder
            .build_struct_gep(layout.vtable_type, vtable_ptr, slot, "fn_ptr_field")
            .map_err(|e| CodegenError::Builder(e.to_string()))?;
        let fn_ptr = self.builder
            .build_load(self.ptr_type(), fn_ptr_field, "fn_ptr")
            .map_err(|e| CodegenError::Builder(e.to_string()))?
            .into_pointer_value();

        let impl_type = self.type_hierarchy
            .find_method_impl_type(type_name, method_name)
            .unwrap_or_else(|| type_name.to_string());
        let static_fn_name = format!("__hulk_method_{}_{}", impl_type, method_name);
        let fn_type = self.module
            .get_function(&static_fn_name)
            .ok_or_else(|| CodegenError::UnknownFunction(static_fn_name.clone()))?
            .get_type();

        let sig = self.type_hierarchy.types
            .get(&impl_type)
            .and_then(|ti| ti.methods.get(method_name))
            .cloned()
            .ok_or_else(|| CodegenError::Unsupported(
                format!("firma de '{}' no encontrada en TypeHierarchy", method_name)))?;

        Ok((fn_ptr, fn_type, sig))
    }

    /// Despacho dinámico para receptores con tipo protocolo.
    /// Carga el type_tag en runtime y genera un switch con un case
    /// por cada tipo concreto que conforma el protocolo.
    pub fn method_dispatch_protocol(
        &mut self,
        obj_ptr:     PointerValue<'ctx>,
        proto_name:  &str,
        method_name: &str,
        user_args:   &[CgValue<'ctx>],
        return_ty:   &HulkType,
    ) -> CodegenResult<CgValue<'ctx>> {
        let conforming: Vec<(String, u32)> = self.type_registry
            .protocol_conformers
            .get(proto_name)
            .cloned()
            .unwrap_or_default();

        if conforming.is_empty() {
            return Err(CodegenError::Unsupported(
                format!("protocolo '{}': sin tipos conformantes registrados en este módulo", proto_name)));
        }

        // Precomputar (tipo_impl, firma) antes del bucle de emisión.
        // type_hierarchy (&self) y builder (&mut self) no pueden coexistir
        // en el mismo cuerpo de bucle. Además, el método a llamar pertenece
        // al tipo que lo define, no necesariamente al conformante declarado.
        let case_data: Vec<(String, FuncSignature)> = conforming.iter()
            .map(|(type_name, _)| {
                let impl_type = self.type_hierarchy
                    .find_method_impl_type(type_name, method_name)
                    .unwrap_or_else(|| type_name.clone());
                let sig = self.type_hierarchy.types
                    .get(&impl_type)
                    .and_then(|ti| ti.methods.get(method_name).cloned())
                    .ok_or_else(|| CodegenError::Unsupported(
                        format!("método '{}' no encontrado en tipo '{}'", method_name, type_name)))?;
                Ok((impl_type, sig))
            })
            .collect::<CodegenResult<_>>()?;

        let i32_ty = self.context.i32_type();
        let runtime_tag = self.builder
            .build_load(i32_ty, obj_ptr, "proto_tag")
            .map_err(|e| CodegenError::Builder(e.to_string()))?
            .into_int_value();

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

        let arms: Vec<(IntValue<'ctx>, BasicBlock<'ctx>)> =
            case_blocks.iter().map(|(v, bb, _)| (*v, *bb)).collect();
        self.builder
            .build_switch(runtime_tag, default_bb, &arms)
            .map_err(|e| CodegenError::Builder(e.to_string()))?;

        self.builder.position_at_end(default_bb);
        self.builder.build_unreachable()
            .map_err(|e| CodegenError::Builder(e.to_string()))?;

        let mut phi_incoming: Vec<(BasicValueEnum<'ctx>, BasicBlock<'ctx>)> = vec![];

        for ((_, case_bb, _type_name), (impl_type, case_sig)) in case_blocks.iter().zip(case_data.iter()) {
            self.builder.position_at_end(*case_bb);

            let mut call_args: Vec<BasicMetadataValueEnum<'ctx>> = vec![obj_ptr.into()];
            for (i, arg_val) in user_args.iter().enumerate() {
                let expected = case_sig.params.get(i)
                    .map(|(_, t)| t.clone())
                    .unwrap_or(HulkType::Object);
                call_args.push(self.coerce_arg(arg_val.clone(), &expected)?);
            }

            let fn_name = format!("__hulk_method_{}_{}", impl_type, method_name);
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
