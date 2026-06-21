use inkwell::FloatPredicate;
use inkwell::values::{BasicValue, BasicMetadataValueEnum, FloatValue, IntValue, PointerValue};

use crate::semantic::HulkType;

use super::context::CodegenContext;
use super::error::{CodegenError, CodegenResult};
use super::value::CgValue;

impl<'ctx> CodegenContext<'ctx> {
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

    pub fn cgvalue_to_str(&self, val: CgValue<'ctx>) -> CodegenResult<PointerValue<'ctx>> {
        match val {
            CgValue::Number(n) => {
                let f = self.require_fn("hulk_str_from_number")?;
                Ok(self.builder
                    .build_call(f, &[n.into()], "num_to_str")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?
                    .try_as_basic_value().left()
                    .ok_or_else(|| CodegenError::Unsupported(
                        "hulk_str_from_number sin valor de retorno".into()))?
                    .into_pointer_value())
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
            CgValue::Vector(_) => {
                Ok(self.builder.build_global_string_ptr("[Vector]", "vec_s")
                    .map_err(|e| CodegenError::Builder(e.to_string()))?.as_pointer_value())
            }
            CgValue::Object(_) => Err(CodegenError::Unsupported(
                "no se puede convertir Object a String sin conocer su tipo — usa print()".into())),
            CgValue::Void => Err(CodegenError::Unsupported(
                "void en contexto de string".into())),
        }
    }

    /// Convierte un CgValue al tipo correcto para pasarlo como argumento de función.
    pub fn coerce_arg(
        &self,
        val:      CgValue<'ctx>,
        expected: &HulkType,
    ) -> CodegenResult<BasicMetadataValueEnum<'ctx>> {
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

    pub fn emit_typed_return(&mut self, val: CgValue<'ctx>, ty: &HulkType) -> CodegenResult<()> {
        if self.is_current_block_terminated() { return Ok(()); }
        match ty {
            HulkType::Number => {
                let v = self.require_number(val)?;
                self.builder.build_return(Some(&v as &dyn BasicValue))
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
            }
            HulkType::Boolean => {
                let v = self.require_bool(val)?;
                self.builder.build_return(Some(&v as &dyn BasicValue))
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
            }
            // NOTA: Null/Unknown NO usan `ret void` aquí. hulk_type_to_llvm()
            // mapea Null/Unknown a `ptr` (nunca a void real de LLVM), así
            // que el tipo declarado de la función SIEMPRE es ptr para estos
            // casos. Antes había una rama que hacía build_return(None) (ret
            // void), lo cual no coincidía con el tipo de retorno declarado
            // y producía: "Function return type does not match operand
            // type of return inst! ret void / ptr". Por eso ahora caen en
            // la rama `_` de abajo, que sí devuelve un puntero (null si no
            // hay valor concreto).
            _ => {
                let ptr: PointerValue = match val {
                    CgValue::Str(p) | CgValue::Object(p) | CgValue::Vector(p) => p,
                    CgValue::Null => self.ptr_type().const_null(),
                    _ => self.ptr_type().const_null(),
                };
                self.builder.build_return(Some(&ptr as &dyn BasicValue))
                    .map_err(|e| CodegenError::Builder(e.to_string()))?;
            }
        }
        Ok(())
    }

    pub fn call_result_to_cgvalue(
        &self,
        call_site: inkwell::values::CallSiteValue<'ctx>,
        ret_ty:    &HulkType,
    ) -> CgValue<'ctx> {
        match ret_ty {
            HulkType::Number => match call_site.try_as_basic_value().left() {
                Some(v) => CgValue::Number(v.into_float_value()),
                None    => CgValue::Number(self.f64_type().const_float(0.0)),
            },
            HulkType::Boolean => match call_site.try_as_basic_value().left() {
                Some(v) => CgValue::Bool(v.into_int_value()),
                None    => CgValue::Bool(self.bool_type().const_int(0, false)),
            },
            HulkType::StringT => match call_site.try_as_basic_value().left() {
                Some(v) => CgValue::Str(v.into_pointer_value()),
                None    => CgValue::Null,
            },
            HulkType::Null | HulkType::Unknown => CgValue::Void,
            _ => match call_site.try_as_basic_value().left() {
                Some(v) => CgValue::Object(v.into_pointer_value()),
                None    => CgValue::Null,
            },
        }
    }
}
