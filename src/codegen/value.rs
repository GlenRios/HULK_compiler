use inkwell::values::{FloatValue, IntValue};

#[derive(Debug, Clone, Copy)]
pub enum CgValue<'ctx> {
    Number(FloatValue<'ctx>),
    Bool(IntValue<'ctx>),
    Void,
}
