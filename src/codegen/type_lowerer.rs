use inkwell::AddressSpace;
use inkwell::types::{BasicTypeEnum, FloatType, IntType, PointerType};

use crate::semantic::HulkType;

use super::context::CodegenContext;

impl<'ctx> CodegenContext<'ctx> {
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
    pub fn hulk_type_to_llvm(&self, ty: &HulkType) -> BasicTypeEnum<'ctx> {
        match ty {
            HulkType::Number  => self.f64_type().into(),
            HulkType::Boolean => self.bool_type().into(),
            _                 => self.ptr_type().into(),
        }
    }

    /// Busca el HulkType de un campo subiendo la cadena de herencia.
    /// TypeInfo.attributes solo guarda los campos PROPIOS; para campos
    /// heredados hay que subir al padre.
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
}
