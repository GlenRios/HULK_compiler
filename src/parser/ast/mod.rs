// src/parser/ast/mod.rs

pub mod span;
pub mod types;
pub mod expr;
pub mod decl;
pub mod program;

// Re-exports de los tipos que el resto del compilador usará directamente
pub use span::Span;
pub use types::TypeName;
pub use program::Program;

// Re-exports de Expr y los nodos más usados para evitar rutas largas
pub use expr::{
    Expr, ExprKind,
    Literal,
    BinaryExpr, BinaryOp,
    UnaryExpr,  UnaryOp,
    PostfixExpr, PostfixOp,
    AssignExpr, AssignOp,
    BlockExpr,
    LetExpr, LetBinding,
    IfExpr, ElifBranch,
    WhileExpr,
    ForExpr,
    NewExpr,
    CallExpr, AccessExpr, MethodCallExpr, IndexExpr,
    VectorExpr,
};

// Re-exports de declaraciones
pub use decl::{
    Decl,
    FuncDecl, Param,
    TypeDecl, TypeMember, AttributeDef, MethodDef,
    ProtocolDecl, MethodSignature,
};

// ─────────────────────────────────────────────────────────────────────────────
//  Tests de integración del módulo ast
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use expr::BinaryOp;

    fn dummy() -> Span { Span::dummy() }

    #[test]
    fn build_simple_program() {
        // print(42);
        let entry = Expr::call(
            Expr::identifier("print", dummy()),
            vec![Expr::number("42", dummy())],
            dummy(),
        );
        let program = Program::new(vec![], entry, dummy());

        assert!(program.declarations.is_empty());
        assert!(matches!(program.entry.kind, ExprKind::Call(_)));
    }

    #[test]
    fn build_binary_expr() {
        // 1 + 2
        let e = Expr::binary(
            BinaryOp::Add,
            Expr::number("1", dummy()),
            Expr::number("2", dummy()),
            dummy(),
        );
        assert!(matches!(e.kind, ExprKind::Binary(_)));
    }

    #[test]
    fn build_let_expr() {
        // let x = 42 in x
        let binding = LetBinding::new("x", None, Expr::number("42", dummy()), dummy());
        let e = Expr::let_expr(
            vec![binding],
            Expr::identifier("x", dummy()),
            dummy(),
        );
        assert!(matches!(e.kind, ExprKind::Let(_)));
        if let ExprKind::Let(let_e) = &e.kind {
            assert_eq!(let_e.bindings.len(), 1);
            assert_eq!(let_e.bindings[0].name, "x");
        }
    }

    #[test]
    fn build_if_expr() {
        // if (true) 1 else 2
        let e = Expr::if_expr(
            Expr::bool(true, dummy()),
            Expr::number("1", dummy()),
            vec![],
            Expr::number("2", dummy()),
            dummy(),
        );
        assert!(matches!(e.kind, ExprKind::If(_)));
        if let ExprKind::If(if_e) = &e.kind {
            assert!(if_e.elif_chain.is_empty());
        }
    }

    #[test]
    fn build_block_expr() {
        // { print(1); print(2) }
        let e = Expr::block(
            vec![
                Expr::call(Expr::identifier("print", dummy()), vec![Expr::number("1", dummy())], dummy()),
                Expr::call(Expr::identifier("print", dummy()), vec![Expr::number("2", dummy())], dummy()),
            ],
            dummy(),
        );
        if let ExprKind::Block(b) = &e.kind {
            assert_eq!(b.body.len(), 2);
            assert!(matches!(b.tail().kind, ExprKind::Call(_)));
        }
    }

    #[test]
    fn build_type_decl() {
        // type Point(x, y) { x = x; }
        let attr = AttributeDef::new("x", None, Expr::identifier("x", dummy()), dummy());
        let decl = TypeDecl::new(
            "Point",
            vec![
                Param::new("x", None, dummy()),
                Param::new("y", None, dummy()),
            ],
            None,
            vec![],
            vec![TypeMember::Attribute(attr)],
            dummy(),
        );
        assert_eq!(decl.name, "Point");
        assert_eq!(decl.type_args.len(), 2);
        assert_eq!(decl.members.len(), 1);
    }

    #[test]
    fn build_func_decl() {
        // function id(x) => x;
        let f = FuncDecl::new(
            "id",
            vec![Param::new("x", None, dummy())],
            None,
            Expr::identifier("x", dummy()),
            dummy(),
        );
        assert_eq!(f.name, "id");
        assert_eq!(f.params.len(), 1);
        assert!(f.return_type.is_none());
    }

    #[test]
    fn build_protocol_decl() {
        // protocol Hashable { hash(): Number; }
        let sig = MethodSignature::new(
            "hash",
            vec![],
            TypeName::simple("Number", dummy()),
            dummy(),
        );
        let proto = ProtocolDecl::new("Hashable", None, vec![sig], dummy());
        assert_eq!(proto.name, "Hashable");
        assert_eq!(proto.methods.len(), 1);
    }

    #[test]
    fn build_vector_explicit() {
        // [1, 2, 3]
        let e = Expr::Vector(Box::new(VectorExpr::explicit(
            vec![
                Expr::number("1", dummy()),
                Expr::number("2", dummy()),
                Expr::number("3", dummy()),
            ],
            dummy(),
        )));
        assert!(matches!(e, Expr::Vector(_)));
    }

    #[test]
    fn build_vector_generator() {
        // [x^2 | x in range(0, 10)]
        let e = Expr::Vector(Box::new(VectorExpr::generator(
            Expr::binary(
                BinaryOp::Power,
                Expr::identifier("x", dummy()),
                Expr::number("2", dummy()),
                dummy(),
            ),
            "x",
            Expr::call(
                Expr::identifier("range", dummy()),
                vec![Expr::number("0", dummy()), Expr::number("10", dummy())],
                dummy(),
            ),
            dummy(),
        )));
        assert!(matches!(e, Expr::Vector(_)));
    }

    #[test]
    fn type_name_display() {
        assert_eq!(TypeName::simple("Number", dummy()).to_string(), "Number");
        assert_eq!(TypeName::vector("Number", dummy()).to_string(), "Number[]");
        assert_eq!(TypeName::iterable("Number", dummy()).to_string(), "Number*");
    }
}