pub mod func_decl;
pub mod type_decl;
pub mod protocol_decl;

pub use func_decl::{FuncDecl, Param};
pub use type_decl::{TypeDecl, TypeMember, AttributeDef, MethodDef};
pub use protocol_decl::{ProtocolDecl, MethodSignature};

use crate::parser::ast::span::Span;

// ─────────────────────────────────────────────────────────────────────────────
//  Decl — unión de todas las declaraciones de nivel superior
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq)]
pub enum Decl {
    Function(FuncDecl),
    Type(TypeDecl),
    Protocol(ProtocolDecl),
}

impl Decl {
    pub fn span(&self) -> Span {
        match self {
            Self::Function(d) => d.span,
            Self::Type(d)     => d.span,
            Self::Protocol(d) => d.span,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Function(d) => &d.name,
            Self::Type(d)     => &d.name,
            Self::Protocol(d) => &d.name,
        }
    }
}