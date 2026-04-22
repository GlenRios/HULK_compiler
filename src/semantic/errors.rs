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
                write!(f, "[{}] Variable '{}' is undefined", span, name),
            Self::UndefinedFunction { name, span } =>
                write!(f, "[{}] Function '{}' is undefined", span, name),
            Self::UndefinedType { name, span } =>
                write!(f, "[{}] Type '{}' is undefined", span, name),
            Self::Redefinition { name, span } =>
                write!(f, "[{}] '{}' is already defined in this scope", span, name),
            Self::TypeMismatch { expected, found, span } =>
                write!(f, "[{}] Expected type '{}', found '{}'", span, expected, found),
            Self::CannotInferType { name, span } =>
                write!(f, "[{}] Cannot infer the type of '{}'", span, name),
            Self::InheritFromPrimitive { type_name, span } =>
                write!(f, "[{}] '{}' cannot inherit from a primitive type", span, type_name),
            Self::CircularInheritance { type_name, span } =>
                write!(f, "[{}] Circular inheritance detected in '{}'", span, type_name),
            Self::WrongArgCount { name, expected, found, span } =>
                write!(f, "[{}] '{}' expects {} argument(s), but {} were given", span, name, expected, found),
            Self::NotCallable { span } =>
                write!(f, "[{}] Expression is not callable", span),
            Self::MethodNotFound { type_name, method, span } =>
                write!(f, "[{}] Type '{}' has no method '{}'", span, type_name, method),
            Self::AttributeNotFound { type_name, attr, span } =>
                write!(f, "[{}] Type '{}' has no attribute '{}'", span, type_name, attr),
            Self::SelfAssignment { span } =>
                write!(f, "[{}] Cannot assign to 'self'", span),
            Self::SelfInInitializer { span } =>
                write!(f, "[{}] 'self' is not available during attribute initialization", span),
            Self::InvalidLValue { span } =>
                write!(f, "[{}] Left-hand side of assignment is not valid", span),
            Self::ProtocolNotConformed { type_name, protocol, missing, span } =>
                write!(f, "[{}] '{}' does not conform to protocol '{}': missing '{}'", span, type_name, protocol, missing),
            Self::OverrideMismatch { method, span } =>
                write!(f, "[{}] Method signature for '{}' does not match the parent method", span, method),
            Self::InvalidCast { from, to, span } =>
                write!(f, "[{}] Cannot cast from '{}' to '{}'", span, from, to),
            Self::InvalidOperandType { op, found, span } =>
                write!(f, "[{}] Operator '{}' cannot be applied to type '{}'", span, op, found),
            Self::InvalidBinaryTypes { op, left, right, span } =>
                write!(f, "[{}] Operator '{}' cannot be applied between '{}' and '{}'", span, op, left, right),
        }
    }
}