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

use std::sync::atomic::{AtomicU32, Ordering};
use crate::parser::ast::span::Span;
use crate::parser::ast::types::TypeName;

// ─────────────────────────────────────────────────────────────────────────────
//  Contador global de IDs de nodo
//  Cada Expr recibe un ID único al construirse.
// ─────────────────────────────────────────────────────────────────────────────
static NEXT_NODE_ID: AtomicU32 = AtomicU32::new(1);

pub fn alloc_node_id() -> u32 {
    NEXT_NODE_ID.fetch_add(1, Ordering::Relaxed)
}

// ─────────────────────────────────────────────────────────────────────────────
//  Expr — nodo raíz de todas las expresiones HULK
//
//  Wrapper struct que envuelve ExprKind añadiendo:
//    • id   — identificador único para el side table de tipos del codegen
//    • span — posición en el fuente (movido aquí desde las variantes inline)
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct Expr {
    pub id:   u32,
    pub kind: ExprKind,
    pub span: Span,
}

// Ignoramos `id` en comparaciones para no romper tests que construyen nodos
impl PartialEq for Expr {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.span == other.span
    }
}

impl Expr {
    pub fn new(kind: ExprKind, span: Span) -> Self {
        Self { id: alloc_node_id(), kind, span }
    }

    // ── Método span() mantenido por compatibilidad con código existente ───────
    pub fn span(&self) -> Span { self.span }

    // ── Constructores de conveniencia ─────────────────────────────────────────

    pub fn number(value: impl Into<String>, span: Span) -> Self {
        Self::new(ExprKind::Literal(Literal::Number { value: value.into(), span }), span)
    }

    pub fn string(value: impl Into<String>, span: Span) -> Self {
        Self::new(ExprKind::Literal(Literal::String { value: value.into(), span }), span)
    }

    pub fn bool(value: bool, span: Span) -> Self {
        Self::new(ExprKind::Literal(Literal::Bool { value, span }), span)
    }

    pub fn null(span: Span) -> Self {
        Self::new(ExprKind::Literal(Literal::Null { span }), span)
    }

    pub fn identifier(name: impl Into<String>, span: Span) -> Self {
        Self::new(ExprKind::Identifier { name: name.into() }, span)
    }

    pub fn binary(op: BinaryOp, left: Expr, right: Expr, span: Span) -> Self {
        Self::new(ExprKind::Binary(Box::new(BinaryExpr::new(op, left, right, span))), span)
    }

    pub fn unary(op: UnaryOp, operand: Expr, span: Span) -> Self {
        Self::new(ExprKind::Unary(Box::new(UnaryExpr::new(op, operand, span))), span)
    }

    pub fn assign(op: AssignOp, target: Expr, value: Expr, span: Span) -> Self {
        Self::new(ExprKind::Assign(Box::new(AssignExpr::new(op, target, value, span))), span)
    }

    pub fn block(body: Vec<Expr>, span: Span) -> Self {
        Self::new(ExprKind::Block(Box::new(BlockExpr::new(body, span))), span)
    }

    pub fn let_expr(bindings: Vec<LetBinding>, body: Expr, span: Span) -> Self {
        Self::new(ExprKind::Let(Box::new(LetExpr::new(bindings, body, span))), span)
    }

    pub fn if_expr(
        cond: Expr, then: Expr, elifs: Vec<ElifBranch>, else_: Expr, span: Span,
    ) -> Self {
        Self::new(ExprKind::If(Box::new(IfExpr::new(cond, then, elifs, else_, span))), span)
    }

    pub fn while_expr(cond: Expr, body: Expr, span: Span) -> Self {
        Self::new(ExprKind::While(Box::new(WhileExpr::new(cond, body, span))), span)
    }

    pub fn for_expr(var: impl Into<String>, iterable: Expr, body: Expr, span: Span) -> Self {
        Self::new(ExprKind::For(Box::new(ForExpr::new(var, iterable, body, span))), span)
    }

    pub fn call(callee: Expr, args: Vec<Expr>, span: Span) -> Self {
        Self::new(ExprKind::Call(Box::new(CallExpr::new(callee, args, span))), span)
    }

    pub fn method_call(object: Expr, method: impl Into<String>, args: Vec<Expr>, span: Span) -> Self {
        Self::new(ExprKind::MethodCall(Box::new(MethodCallExpr::new(object, method, args, span))), span)
    }

    pub fn access(object: Expr, field: impl Into<String>, span: Span) -> Self {
        Self::new(ExprKind::Access(Box::new(AccessExpr::new(object, field, span))), span)
    }

    pub fn index(collection: Expr, idx: Expr, span: Span) -> Self {
        Self::new(ExprKind::Index(Box::new(IndexExpr::new(collection, idx, span))), span)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  ExprKind — variantes sintácticas de las expresiones HULK
//
//  Las variantes que antes tenían `span` inline (Identifier, Base, Is, As)
//  ya no lo tienen — el span vive en el wrapper Expr.
//  Las variantes que usan Box<SomeStruct> conservan sus structs sin cambios.
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    // ── Átomos ────────────────────────────────────────────────────────────
    /// Literal: número, string, char, bool, null
    Literal(Literal),

    /// Variable o función global: `x`, `myVar`, `print`
    Identifier { name: String },

    /// `base` — referencia al método padre dentro de un método
    Base,

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
    Is { expr: Box<Expr>, type_name: TypeName },

    /// Downcast: `expr as TypeName`
    As { expr: Box<Expr>, type_name: TypeName },

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
