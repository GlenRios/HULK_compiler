// src/semantic/errors.rs

use crate::parser::ast::Span;
use std::fmt;

#[derive(Debug, Clone)]
pub enum SemanticError {
    // ── Variables y scope ─────────────────────────────────────────────────
    UndefinedVariable    { name: String, span: Span },
    UndefinedFunction    { name: String, span: Span },
    UndefinedType        { name: String, span: Span },
    Redefinition         { name: String, span: Span },

    // ── Tipos ─────────────────────────────────────────────────────────────
    TypeMismatch         { expected: String, found: String, span: Span },
    CannotInferType      { name: String, span: Span },
    InheritFromPrimitive { type_name: String, span: Span },
    CircularInheritance  { type_name: String, span: Span },

    // ── Llamadas ──────────────────────────────────────────────────────────
    WrongArgCount        { name: String, expected: usize, found: usize, span: Span },
    NotCallable          { span: Span },
    MethodNotFound       { type_name: String, method: String, span: Span },
    AttributeNotFound    { type_name: String, attr: String, span: Span },

    // ── Semántica especial ────────────────────────────────────────────────
    SelfAssignment       { span: Span },
    SelfInInitializer    { span: Span },
    InvalidLValue        { span: Span },
    ProtocolNotConformed { type_name: String, protocol: String, missing: String, span: Span },
    OverrideMismatch     { method: String, span: Span },
    InvalidCast          { from: String, to: String, span: Span },

    // ── Operadores ────────────────────────────────────────────────────────
    InvalidOperandType   { op: String, found: String, span: Span },
    InvalidBinaryTypes   { op: String, left: String, right: String, span: Span },
}

impl SemanticError {
    pub fn span(&self) -> Span {
        match self {
            Self::UndefinedVariable    { span, .. } => *span,
            Self::UndefinedFunction    { span, .. } => *span,
            Self::UndefinedType        { span, .. } => *span,
            Self::Redefinition         { span, .. } => *span,
            Self::TypeMismatch         { span, .. } => *span,
            Self::CannotInferType      { span, .. } => *span,
            Self::InheritFromPrimitive { span, .. } => *span,
            Self::CircularInheritance  { span, .. } => *span,
            Self::WrongArgCount        { span, .. } => *span,
            Self::NotCallable          { span }     => *span,
            Self::MethodNotFound       { span, .. } => *span,
            Self::AttributeNotFound    { span, .. } => *span,
            Self::SelfAssignment       { span }     => *span,
            Self::SelfInInitializer    { span }     => *span,
            Self::InvalidLValue        { span }     => *span,
            Self::ProtocolNotConformed { span, .. } => *span,
            Self::OverrideMismatch     { span, .. } => *span,
            Self::InvalidCast          { span, .. } => *span,
            Self::InvalidOperandType   { span, .. } => *span,
            Self::InvalidBinaryTypes   { span, .. } => *span,
        }
    }
}

impl fmt::Display for SemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UndefinedVariable { name, span } =>
                write!(f, "[{}] Variable '{}' no definida", span, name),
            Self::UndefinedFunction { name, span } =>
                write!(f, "[{}] Función '{}' no definida", span, name),
            Self::UndefinedType { name, span } =>
                write!(f, "[{}] Tipo '{}' no definido", span, name),
            Self::Redefinition { name, span } =>
                write!(f, "[{}] '{}' ya está definido en este scope", span, name),
            Self::TypeMismatch { expected, found, span } =>
                write!(f, "[{}] Se esperaba tipo '{}', se encontró '{}'", span, expected, found),
            Self::CannotInferType { name, span } =>
                write!(f, "[{}] No se puede inferir el tipo de '{}'", span, name),
            Self::InheritFromPrimitive { type_name, span } =>
                write!(f, "[{}] '{}' no puede heredar de un tipo primitivo", span, type_name),
            Self::CircularInheritance { type_name, span } =>
                write!(f, "[{}] Herencia circular detectada en '{}'", span, type_name),
            Self::WrongArgCount { name, expected, found, span } =>
                write!(f, "[{}] '{}' espera {} argumento(s), se dieron {}", span, name, expected, found),
            Self::NotCallable { span } =>
                write!(f, "[{}] La expresión no es invocable", span),
            Self::MethodNotFound { type_name, method, span } =>
                write!(f, "[{}] El tipo '{}' no tiene método '{}'", span, type_name, method),
            Self::AttributeNotFound { type_name, attr, span } =>
                write!(f, "[{}] El tipo '{}' no tiene atributo '{}'", span, type_name, attr),
            Self::SelfAssignment { span } =>
                write!(f, "[{}] No se puede asignar a 'self'", span),
            Self::SelfInInitializer { span } =>
                write!(f, "[{}] 'self' no está disponible en la inicialización de atributos", span),
            Self::InvalidLValue { span } =>
                write!(f, "[{}] El lado izquierdo de la asignación no es válido", span),
            Self::ProtocolNotConformed { type_name, protocol, missing, span } =>
                write!(f, "[{}] '{}' no cumple el protocolo '{}': falta '{}'", span, type_name, protocol, missing),
            Self::OverrideMismatch { method, span } =>
                write!(f, "[{}] La firma del método '{}' no coincide con la del padre", span, method),
            Self::InvalidCast { from, to, span } =>
                write!(f, "[{}] No se puede hacer cast de '{}' a '{}'", span, from, to),
            Self::InvalidOperandType { op, found, span } =>
                write!(f, "[{}] Operador '{}' no aplicable a tipo '{}'", span, op, found),
            Self::InvalidBinaryTypes { op, left, right, span } =>
                write!(f, "[{}] Operador '{}' no aplicable entre '{}' y '{}'", span, op, left, right),
        }
    }
}