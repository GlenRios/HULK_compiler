use crate::parser::ast::span::Span;
use crate::parser::ast::types::TypeName;
use super::func_decl::Param;

/// Firma de método en un protocolo.
///
/// Los métodos de protocolo SIEMPRE tienen anotación de tipo de retorno.
///
/// ```text
/// hash(): Number;
/// equals(other: Object): Boolean;
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct MethodSignature {
    pub name:        String,
    pub params:      Vec<Param>,
    pub return_type: TypeName,   // obligatorio en protocolos
    pub span:        Span,
}

impl MethodSignature {
    pub fn new(
        name:        impl Into<String>,
        params:      Vec<Param>,
        return_type: TypeName,
        span:        Span,
    ) -> Self {
        Self { name: name.into(), params, return_type, span }
    }
}

/// Declaración de protocolo.
///
/// ```text
/// protocol Equatable extends Hashable {
///     equals(other: Object): Boolean;
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolDecl {
    pub name:    String,
    /// Protocolo del que extiende, si lo hay.
    pub extends: Option<TypeName>,
    pub methods: Vec<MethodSignature>,
    pub span:    Span,
}

impl ProtocolDecl {
    pub fn new(
        name:    impl Into<String>,
        extends: Option<TypeName>,
        methods: Vec<MethodSignature>,
        span:    Span,
    ) -> Self {
        Self { name: name.into(), extends, methods, span }
    }
}