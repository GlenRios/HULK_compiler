pub mod literal;
pub mod binary;
pub mod unary;
pub mod assign;
pub mod block;
pub mod let_expr;
pub mod if_expr;
pub mod while_expr;
pub mod for_expr;
pub mod new_expr;
pub mod call_access;
pub mod vector;

// Re-exports para que el resto del compilador use `expr::Literal` etc.
pub use literal::Literal;
pub use binary::{BinaryExpr, BinaryOp};
pub use unary::{UnaryExpr, UnaryOp, PostfixExpr, PostfixOp};
pub use assign::{AssignExpr, AssignOp};
pub use block::BlockExpr;
pub use let_expr::{LetExpr, LetBinding};
pub use if_expr::{IfExpr, ElifBranch};
pub use while_expr::WhileExpr;
pub use for_expr::ForExpr;
pub use new_expr::NewExpr;
pub use call_access::{CallExpr, AccessExpr, MethodCallExpr, IndexExpr};
pub use vector::VectorExpr;

use crate::parser::ast::span::Span;
use crate::parser::ast::types::TypeName;

// ─────────────────────────────────────────────────────────────────────────────
//  Expr — nodo raíz de todas las expresiones HULK
//
//  Cada variante contiene el struct que la describe en su propio archivo.
//  Los tipos complejos van en Box<> para mantener Expr de tamaño fijo.
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    // ── Átomos ────────────────────────────────────────────────────────────
    /// Literal: número, string, char, bool, null
    Literal(Literal),

    /// Variable o función global: `x`, `myVar`, `print`
    Identifier { name: String, span: Span },

    /// `base` — referencia al método padre dentro de un método
    Base(Span),

    // ── Operaciones ───────────────────────────────────────────────────────
    /// Operación binaria: `a + b`, `x == y`, `s @ t`
    Binary(Box<BinaryExpr>),

    /// Operación unaria prefija: `-x`, `!flag`
    Unary(Box<UnaryExpr>),

    /// Operación postfija: `x++`, `x--`
    Postfix(Box<PostfixExpr>),

    /// Asignación destructiva: `x := e`, `x += e`
    Assign(Box<AssignExpr>),

    // ── is / as ───────────────────────────────────────────────────────────
    /// Test de tipo: `expr is TypeName`
    Is { expr: Box<Expr>, type_name: TypeName, span: Span },

    /// Downcast: `expr as TypeName`
    As { expr: Box<Expr>, type_name: TypeName, span: Span },

    // ── Llamadas y accesos ────────────────────────────────────────────────
    /// Llamada a función/functor: `f(args)`
    Call(Box<CallExpr>),

    /// Acceso a atributo: `obj.field`
    Access(Box<AccessExpr>),

    /// Llamada a método: `obj.method(args)`
    MethodCall(Box<MethodCallExpr>),

    /// Indexación: `v[i]`
    Index(Box<IndexExpr>),

    // ── Expresiones compuestas ────────────────────────────────────────────
    /// Bloque: `{ e1; e2; e3 }`
    Block(Box<BlockExpr>),

    /// Let: `let x = e in body`
    Let(Box<LetExpr>),

    /// If: `if (c) t [elif (c) t]* else e`
    If(Box<IfExpr>),

    /// While: `while (c) body`
    While(Box<WhileExpr>),

    /// For: `for (x in iter) body`
    For(Box<ForExpr>),

    /// Instanciación: `new Type(args)`
    New(Box<NewExpr>),

    /// Vector: `[e1, e2]` o `[expr | x in iter]`
    Vector(Box<VectorExpr>),
}

impl Expr {
    // ── Constructores de conveniencia ─────────────────────────────────────

    pub fn number(value: impl Into<String>, span: Span) -> Self {
        Self::Literal(Literal::Number { value: value.into(), span })
    }

    pub fn string(value: impl Into<String>, span: Span) -> Self {
        Self::Literal(Literal::String { value: value.into(), span })
    }

    pub fn bool(value: bool, span: Span) -> Self {
        Self::Literal(Literal::Bool { value, span })
    }

    pub fn null(span: Span) -> Self {
        Self::Literal(Literal::Null { span })
    }

    pub fn identifier(name: impl Into<String>, span: Span) -> Self {
        Self::Identifier { name: name.into(), span }
    }

    pub fn binary(op: BinaryOp, left: Expr, right: Expr, span: Span) -> Self {
        Self::Binary(Box::new(BinaryExpr::new(op, left, right, span)))
    }

    pub fn unary(op: UnaryOp, operand: Expr, span: Span) -> Self {
        Self::Unary(Box::new(UnaryExpr::new(op, operand, span)))
    }

    pub fn assign(op: AssignOp, target: Expr, value: Expr, span: Span) -> Self {
        Self::Assign(Box::new(AssignExpr::new(op, target, value, span)))
    }

    pub fn block(body: Vec<Expr>, span: Span) -> Self {
        Self::Block(Box::new(BlockExpr::new(body, span)))
    }

    pub fn let_expr(bindings: Vec<LetBinding>, body: Expr, span: Span) -> Self {
        Self::Let(Box::new(LetExpr::new(bindings, body, span)))
    }

    pub fn if_expr(
        cond: Expr, then: Expr, elifs: Vec<ElifBranch>, else_: Expr, span: Span,
    ) -> Self {
        Self::If(Box::new(IfExpr::new(cond, then, elifs, else_, span)))
    }

    pub fn while_expr(cond: Expr, body: Expr, span: Span) -> Self {
        Self::While(Box::new(WhileExpr::new(cond, body, span)))
    }

    pub fn for_expr(var: impl Into<String>, iterable: Expr, body: Expr, span: Span) -> Self {
        Self::For(Box::new(ForExpr::new(var, iterable, body, span)))
    }

    pub fn call(callee: Expr, args: Vec<Expr>, span: Span) -> Self {
        Self::Call(Box::new(CallExpr::new(callee, args, span)))
    }

    pub fn method_call(object: Expr, method: impl Into<String>, args: Vec<Expr>, span: Span) -> Self {
        Self::MethodCall(Box::new(MethodCallExpr::new(object, method, args, span)))
    }

    pub fn access(object: Expr, field: impl Into<String>, span: Span) -> Self {
        Self::Access(Box::new(AccessExpr::new(object, field, span)))
    }

    pub fn index(collection: Expr, idx: Expr, span: Span) -> Self {
        Self::Index(Box::new(IndexExpr::new(collection, idx, span)))
    }

    // ── Utilidades ────────────────────────────────────────────────────────

    pub fn span(&self) -> Span {
        match self {
            Self::Literal(l)             => l.span(),
            Self::Identifier { span, .. } => *span,
            Self::Base(span)             => *span,
            Self::Binary(e)              => e.span,
            Self::Unary(e)               => e.span,
            Self::Postfix(e)             => e.span,
            Self::Assign(e)              => e.span,
            Self::Is { span, .. }        => *span,
            Self::As { span, .. }        => *span,
            Self::Call(e)                => e.span,
            Self::Access(e)              => e.span,
            Self::MethodCall(e)          => e.span,
            Self::Index(e)               => e.span,
            Self::Block(e)               => e.span,
            Self::Let(e)                 => e.span,
            Self::If(e)                  => e.span,
            Self::While(e)               => e.span,
            Self::For(e)                 => e.span,
            Self::New(e)                 => e.span,
            Self::Vector(e)              => e.span(),
        }
    }
}