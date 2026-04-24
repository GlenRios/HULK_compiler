use inkwell::values::{FloatValue, IntValue, PointerValue};
use crate::semantic::HulkType;

#[derive(Debug, Clone, Copy)]
pub enum CgValue<'ctx> {
    Number(FloatValue<'ctx>),    // f64  — HULK Number
    Bool(IntValue<'ctx>),        // i1   — HULK Boolean
    Str(PointerValue<'ctx>),     // ptr  — HULK String (i8* null-terminated)
    Object(PointerValue<'ctx>),  // ptr  — HULK tipo de usuario o Object
    Vector(PointerValue<'ctx>),  // ptr  — HULK Vector
    Null,                        // sentinel Rust — null pointer, se emite como ptr null
    Void,                        // sentinel Rust — sin valor (while, block vacío, etc.)
}

impl<'ctx> CgValue<'ctx> {
    /// Deriva el HulkType semántico desde el variant.
    /// Usado para saber qué tipo de slot alloca al guardar en una variable.
    pub fn hulk_type(&self) -> HulkType {
        match self {
            Self::Number(_) => HulkType::Number,
            Self::Bool(_)   => HulkType::Boolean,
            Self::Str(_)    => HulkType::StringT,
            Self::Object(_) => HulkType::Object,
            Self::Vector(_) => HulkType::Object,  // Vector se trata como Object para el slot
            Self::Null      => HulkType::Null,
            Self::Void      => HulkType::Null,
        }
    }
}
