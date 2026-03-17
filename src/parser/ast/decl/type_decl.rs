use crate::parser::ast::span::Span;
use crate::parser::ast::types::TypeName;
use crate::parser::ast::expr::Expr;
use super::func_decl::Param;

/// Definición de atributo dentro de un tipo.
///
/// ```text
/// x = 0;
/// x: Number = 0;
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct AttributeDef {
    pub name:     String,
    pub type_ann: Option<TypeName>,
    pub value:    Box<Expr>,
    pub span:     Span,
}

impl AttributeDef {
    pub fn new(
        name:     impl Into<String>,
        type_ann: Option<TypeName>,
        value:    Expr,
        span:     Span,
    ) -> Self {
        Self { name: name.into(), type_ann, value: Box::new(value), span }
    }
}

/// Definición de método dentro de un tipo.
///
/// Igual que `FuncDecl` pero sin la palabra clave `function`.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodDef {
    pub name:        String,
    pub params:      Vec<Param>,
    pub return_type: Option<TypeName>,
    pub body:        Box<Expr>,
    pub span:        Span,
}

impl MethodDef {
    pub fn new(
        name:        impl Into<String>,
        params:      Vec<Param>,
        return_type: Option<TypeName>,
        body:        Expr,
        span:        Span,
    ) -> Self {
        Self { name: name.into(), params, return_type, body: Box::new(body), span }
    }
}

/// Miembro de un tipo: atributo o método.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeMember {
    Attribute(AttributeDef),
    Method(MethodDef),
}

/// Declaración de tipo.
///
/// ```text
/// type Point(x: Number, y: Number) inherits Object {
///     x: Number = x;
///     getX() => self.x;
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TypeDecl {
    pub name:          String,
    /// Argumentos del constructor: `type Point(x, y)`
    pub type_args:     Vec<Param>,
    /// Herencia: `inherits Point(rho * sin(phi), …)`
    pub parent:        Option<TypeName>,
    pub parent_args:   Vec<Expr>,
    pub members:       Vec<TypeMember>,
    pub span:          Span,
}

impl TypeDecl {
    pub fn new(
        name:        impl Into<String>,
        type_args:   Vec<Param>,
        parent:      Option<TypeName>,
        parent_args: Vec<Expr>,
        members:     Vec<TypeMember>,
        span:        Span,
    ) -> Self {
        Self { name: name.into(), type_args, parent, parent_args, members, span }
    }
}