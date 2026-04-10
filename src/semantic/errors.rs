use crate::parser::ast::Span;

#[derive(Debug, Clone)]
pub enum SemanticError {
    // Variables y scoping
    UndefinedVariable        { name: String, span: Span },
    UndefinedFunction        { name: String, span: Span },
    UndefinedType            { name: String, span: Span },
    Redefinition             { name: String, span: Span },

    // Tipos
    TypeMismatch             { expected: String, found: String, span: Span },
    CannotInferType          { name: String, span: Span },
    InheritFromPrimitive     { type_name: String, span: Span },
    CircularInheritance      { type_name: String, span: Span },

    // Llamadas
    WrongArgCount            { name: String, expected: usize, found: usize, span: Span },
    NotCallable              { span: Span },
    MethodNotFound           { type_name: String, method: String, span: Span },
    AttributeNotFound        { type_name: String, attr: String, span: Span },

    // Semántica especial
    SelfAssignment           { span: Span },       // self := ...
    SelfInInitializer        { span: Span },       // self en atributo
    InvalidLValue            { span: Span },
    ProtocolNotConformed     { type_name: String, protocol: String, missing: String, span: Span },
    OverrideMismatch         { method: String, span: Span },
    DowncastFailed           { from: String, to: String, span: Span },

    // Operadores
    InvalidOperandType       { op: String, found: String, span: Span },
    InvalidBinaryTypes       { op: String, left: String, right: String, span: Span },
}

impl SemanticError {
    pub fn span(&self) -> Span { /* match en todos los campos span */ todo!() }
}

impl std::fmt::Display for SemanticError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::UndefinedVariable { name, span } =>
                write!(f, "[{}] Variable '{}' no definida", span, name),
            Self::TypeMismatch { expected, found, span } =>
                write!(f, "[{}] Tipo esperado '{}', encontrado '{}'", span, expected, found),
            Self::InheritFromPrimitive { type_name, span } =>
                write!(f, "[{}] '{}' no puede heredar de un tipo primitivo", span, type_name),
            Self::SelfAssignment { span } =>
                write!(f, "[{}] No se puede asignar a 'self'", span),
            // ... resto de variantes
            _ => write!(f, "{:?}", self),
        }
    }
}