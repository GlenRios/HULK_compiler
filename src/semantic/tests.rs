// src/semantic/tests.rs
//
// Suite completa de tests del analizador semántico.
// Cada test construye un AST directamente (sin pasar por el parser)
// y verifica que el TypeChecker produce los errores esperados — o ninguno.
//
// Convención:
//   check_ok!(program)         → el programa debe pasar sin errores
//   check_err!(program, Kind)  → debe producir al menos un error de ese Kind

#[cfg(test)]
mod tests {
    use crate::parser::ast::{
        Decl, Expr, FuncDecl, Param, Program, Span,
        TypeDecl, TypeMember, AttributeDef, MethodDef,
        ProtocolDecl, MethodSignature,
        BinaryOp, UnaryOp, AssignOp, PostfixOp,
        LetBinding, ElifBranch,
        TypeName, VectorExpr, NewExpr,
    };
    use crate::semantic::{analyze, SemanticError};

    // ─────────────────────────────────────────────────────────────────────────
    //  Helpers
    // ─────────────────────────────────────────────────────────────────────────

    fn d() -> Span { Span::dummy() }

    fn num(v: &str)   -> Expr { Expr::number(v, d()) }
    fn str_(v: &str)  -> Expr { Expr::string(v, d()) }
    fn bool_(v: bool) -> Expr { Expr::bool(v, d()) }
    fn null()         -> Expr { Expr::null(d()) }
    fn id(n: &str)    -> Expr { Expr::identifier(n, d()) }

    fn add(l: Expr, r: Expr) -> Expr { Expr::binary(BinaryOp::Add, l, r, d()) }
    fn sub(l: Expr, r: Expr) -> Expr { Expr::binary(BinaryOp::Sub, l, r, d()) }
    fn and(l: Expr, r: Expr) -> Expr { Expr::binary(BinaryOp::And, l, r, d()) }
    fn or_(l: Expr, r: Expr) -> Expr { Expr::binary(BinaryOp::Or,  l, r, d()) }
    fn eq_(l: Expr, r: Expr) -> Expr { Expr::binary(BinaryOp::Eq,  l, r, d()) }
    fn lt_(l: Expr, r: Expr) -> Expr { Expr::binary(BinaryOp::Less, l, r, d()) }
    fn cat(l: Expr, r: Expr) -> Expr { Expr::binary(BinaryOp::Concat, l, r, d()) }

    fn neg(e: Expr) -> Expr { Expr::unary(UnaryOp::Neg, e, d()) }
    fn not(e: Expr) -> Expr { Expr::unary(UnaryOp::Not, e, d()) }

    fn block(exprs: Vec<Expr>) -> Expr { Expr::block(exprs, d()) }

    fn let_(bindings: Vec<(&str, Option<TypeName>, Expr)>, body: Expr) -> Expr {
        let bs = bindings.into_iter()
            .map(|(n, t, v)| LetBinding::new(n, t, v, d()))
            .collect();
        Expr::let_expr(bs, body, d())
    }

    fn if_(cond: Expr, then: Expr, else_: Expr) -> Expr {
        Expr::if_expr(cond, then, vec![], else_, d())
    }

    fn if_elif(cond: Expr, then: Expr, elifs: Vec<(Expr, Expr)>, else_: Expr) -> Expr {
        let elif_chain = elifs.into_iter()
            .map(|(c, b)| ElifBranch::new(c, b, d()))
            .collect();
        Expr::if_expr(cond, then, elif_chain, else_, d())
    }

    fn while_(cond: Expr, body: Expr) -> Expr {
        Expr::while_expr(cond, body, d())
    }

    fn for_(var: &str, iter: Expr, body: Expr) -> Expr {
        Expr::for_expr(var, iter, body, d())
    }

    fn call(name: &str, args: Vec<Expr>) -> Expr {
        Expr::call(id(name), args, d())
    }

    fn method_call(obj: Expr, method: &str, args: Vec<Expr>) -> Expr {
        Expr::method_call(obj, method, args, d())
    }

    fn access(obj: Expr, field: &str) -> Expr {
        Expr::access(obj, field, d())
    }

    fn assign(target: Expr, value: Expr) -> Expr {
        Expr::assign(AssignOp::Assign, target, value, d())
    }

    fn new_(type_name: &str, args: Vec<Expr>) -> Expr {
        Expr::New(Box::new(NewExpr::new(TypeName::simple(type_name, d()), args, d())))
    }

    fn is_expr(e: Expr, t: &str) -> Expr {
        Expr::Is { expr: Box::new(e), type_name: TypeName::simple(t, d()), span: d() }
    }

    fn as_expr(e: Expr, t: &str) -> Expr {
        Expr::As { expr: Box::new(e), type_name: TypeName::simple(t, d()), span: d() }
    }

    fn vec_explicit(elems: Vec<Expr>) -> Expr {
        Expr::Vector(Box::new(VectorExpr::explicit(elems, d())))
    }

    fn vec_gen(body: Expr, var: &str, iter: Expr) -> Expr {
        Expr::Vector(Box::new(VectorExpr::generator(body, var, iter, d())))
    }

    fn ty(name: &str) -> TypeName { TypeName::simple(name, d()) }

    fn simple_program(entry: Expr) -> Program {
        Program::new(vec![], entry, d())
    }

    fn program_with_decls(decls: Vec<Decl>, entry: Expr) -> Program {
        Program::new(decls, entry, d())
    }

    fn func_decl(
        name: &str,
        params: Vec<(&str, Option<TypeName>)>,
        ret: Option<TypeName>,
        body: Expr,
    ) -> Decl {
        let ps = params.into_iter()
            .map(|(n, t)| Param::new(n, t, d()))
            .collect();
        Decl::Function(FuncDecl::new(name, ps, ret, body, d()))
    }

    // Helper modificado: ahora acepta parent_args
    fn type_decl(
        name: &str,
        args: Vec<(&str, Option<TypeName>)>,
        parent: Option<&str>,
        parent_args: Vec<Expr>,
        members: Vec<TypeMember>,
    ) -> Decl {
        let ps = args.into_iter().map(|(n, t)| Param::new(n, t, d())).collect();
        let par = parent.map(|p| ty(p));
        Decl::Type(TypeDecl::new(name, ps, par, parent_args, members, d()))
    }

    fn attr(name: &str, ann: Option<TypeName>, value: Expr) -> TypeMember {
        TypeMember::Attribute(AttributeDef::new(name, ann, value, d()))
    }

    fn method(
        name: &str,
        params: Vec<(&str, Option<TypeName>)>,
        ret: Option<TypeName>,
        body: Expr,
    ) -> TypeMember {
        let ps = params.into_iter().map(|(n, t)| Param::new(n, t, d())).collect();
        TypeMember::Method(MethodDef::new(name, ps, ret, body, d()))
    }

    fn protocol_decl(name: &str, extends: Option<&str>, methods: Vec<(&str, TypeName)>) -> Decl {
        let sigs = methods.into_iter()
            .map(|(mn, ret)| MethodSignature::new(mn, vec![], ret, d()))
            .collect();
        let ext = extends.map(|e| ty(e));
        Decl::Protocol(ProtocolDecl::new(name, ext, sigs, d()))
    }

    // ── Macros de verificación ────────────────────────────────────────────────

    macro_rules! check_ok {
        ($prog:expr) => {{
            let errors = match analyze(&$prog) {
                Ok(())      => vec![],
                Err(errors) => errors,
            };
            assert!(
                errors.is_empty(),
                "Se esperaba OK pero hubo errores:\n{}",
                errors.iter().map(|e| format!("  • {}", e)).collect::<Vec<_>>().join("\n")
            );
        }};
    }

    macro_rules! check_err {
        ($prog:expr, $kind:pat) => {{
            let errors = match analyze(&$prog) {
                Ok(())      => vec![],
                Err(errors) => errors,
            };
            assert!(
                errors.iter().any(|e| matches!(e, $kind)),
                "Se esperaba un error del tipo `{}` pero los errores fueron:\n{}",
                stringify!($kind),
                errors.iter().map(|e| format!("  • {}", e)).collect::<Vec<_>>().join("\n")
            );
        }};
    }

    macro_rules! check_no_err {
        ($prog:expr, $kind:pat) => {{
            let errors = match analyze(&$prog) {
                Ok(())      => vec![],
                Err(errors) => errors,
            };
            assert!(
                !errors.iter().any(|e| matches!(e, $kind)),
                "No se esperaba un error del tipo `{}` pero apareció:\n{}",
                stringify!($kind),
                errors.iter().map(|e| format!("  • {}", e)).collect::<Vec<_>>().join("\n")
            );
        }};
    }

    macro_rules! error_count {
        ($prog:expr) => {{
            match analyze(&$prog) {
                Ok(())      => 0,
                Err(errors) => errors.len(),
            }
        }};
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  1. LITERALES
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn literal_number_ok() {
        check_ok!(simple_program(num("42")));
    }

    #[test]
    fn literal_string_ok() {
        check_ok!(simple_program(str_("hola")));
    }

    #[test]
    fn literal_bool_ok() {
        check_ok!(simple_program(bool_(true)));
    }

    #[test]
    fn literal_null_ok() {
        check_ok!(simple_program(null()));
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  2. IDENTIFICADORES Y SCOPE
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn identifier_undefined_error() {
        check_err!(
            simple_program(id("x")),
            SemanticError::UndefinedVariable { .. }
        );
    }

    #[test]
    fn identifier_builtin_pi_ok() {
        check_ok!(simple_program(id("PI")));
    }

    #[test]
    fn identifier_builtin_e_ok() {
        check_ok!(simple_program(id("E")));
    }

    #[test]
    fn identifier_true_false_ok() {
        check_ok!(simple_program(id("true")));
        check_ok!(simple_program(id("false")));
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  3. OPERACIONES BINARIAS
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn binary_arithmetic_ok() {
        check_ok!(simple_program(add(num("1"), num("2"))));
        check_ok!(simple_program(sub(num("10"), num("5"))));
        check_ok!(simple_program(Expr::binary(BinaryOp::Mul, num("3"), num("4"), d())));
        check_ok!(simple_program(Expr::binary(BinaryOp::Div, num("8"), num("2"), d())));
        check_ok!(simple_program(Expr::binary(BinaryOp::Mod, num("7"), num("3"), d())));
        check_ok!(simple_program(Expr::binary(BinaryOp::Power, num("2"), num("10"), d())));
    }

    #[test]
    fn binary_arithmetic_wrong_types() {
        // String + Number → error
        check_err!(
            simple_program(add(str_("hola"), num("1"))),
            SemanticError::InvalidBinaryTypes { .. }
        );
    }

    #[test]
    fn binary_arithmetic_bool_error() {
        // Bool - Number → error
        check_err!(
            simple_program(sub(bool_(true), num("1"))),
            SemanticError::InvalidBinaryTypes { .. }
        );
    }

    #[test]
    fn binary_logical_ok() {
        check_ok!(simple_program(and(bool_(true), bool_(false))));
        check_ok!(simple_program(or_(bool_(true), bool_(true))));
    }

    #[test]
    fn binary_logical_wrong_types() {
        check_err!(
            simple_program(and(num("1"), bool_(true))),
            SemanticError::InvalidBinaryTypes { .. }
        );
    }

    #[test]
    fn binary_comparison_numbers_ok() {
        check_ok!(simple_program(lt_(num("1"), num("2"))));
        check_ok!(simple_program(Expr::binary(BinaryOp::Greater,   num("5"), num("3"), d())));
        check_ok!(simple_program(Expr::binary(BinaryOp::LessEq,    num("1"), num("1"), d())));
        check_ok!(simple_program(Expr::binary(BinaryOp::GreaterEq, num("2"), num("1"), d())));
    }

    #[test]
    fn binary_comparison_string_error() {
        check_err!(
            simple_program(lt_(str_("a"), str_("b"))),
            SemanticError::InvalidBinaryTypes { .. }
        );
    }

    #[test]
    fn binary_equality_same_types_ok() {
        check_ok!(simple_program(eq_(num("1"), num("1"))));
        check_ok!(simple_program(eq_(bool_(true), bool_(false))));
    }

    #[test]
    fn binary_equality_incompatible_types_error() {
        check_err!(
            simple_program(eq_(num("1"), str_("x"))),
            SemanticError::InvalidBinaryTypes { .. }
        );
    }

    #[test]
    fn binary_concat_ok() {
        // @ acepta cualquier tipo → String
        check_ok!(simple_program(cat(str_("hola"), num("42"))));
        check_ok!(simple_program(cat(str_("x"), str_("y"))));
        check_ok!(simple_program(Expr::binary(BinaryOp::DoubleConcat, str_("a"), str_("b"), d())));
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  4. OPERACIONES UNARIAS Y POSTFIJAS
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn unary_neg_number_ok() {
        check_ok!(simple_program(neg(num("5"))));
    }

    #[test]
    fn unary_neg_wrong_type() {
        check_err!(
            simple_program(neg(bool_(true))),
            SemanticError::InvalidOperandType { .. }
        );
    }

    #[test]
    fn unary_not_bool_ok() {
        check_ok!(simple_program(not(bool_(true))));
    }

    #[test]
    fn unary_not_wrong_type() {
        check_err!(
            simple_program(not(num("1"))),
            SemanticError::InvalidOperandType { .. }
        );
    }

    #[test]
    fn postfix_increment_number_ok() {
        let prog = program_with_decls(
            vec![],
            let_(vec![("x", None, num("0"))],
                Expr::Postfix(Box::new(
                    crate::parser::ast::PostfixExpr::new(PostfixOp::Increment, id("x"), d())
                )))
        );
        check_ok!(prog);
    }

    #[test]
    fn postfix_on_bool_error() {
        check_err!(
            simple_program(Expr::Postfix(Box::new(
                crate::parser::ast::PostfixExpr::new(PostfixOp::Increment, bool_(true), d())
            ))),
            SemanticError::InvalidOperandType { .. }
        );
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  5. BLOQUE
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn block_type_is_last_expr() {
        // { 1; "hola"; true } → Boolean
        // No debe lanzar errores
        check_ok!(simple_program(block(vec![num("1"), str_("hola"), bool_(true)])));
    }

    #[test]
    fn block_single_expr_ok() {
        check_ok!(simple_program(block(vec![num("42")])));
    }

    #[test]
    fn block_propagates_inner_errors() {
        check_err!(
            simple_program(block(vec![num("1"), id("undefined_var")])),
            SemanticError::UndefinedVariable { .. }
        );
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  6. LET
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn let_simple_ok() {
        // let x = 42 in x
        check_ok!(simple_program(let_(vec![("x", None, num("42"))], id("x"))));
    }

    #[test]
    fn let_with_type_annotation_ok() {
        // let x: Number = 42 in x
        check_ok!(simple_program(
            let_(vec![("x", Some(ty("Number")), num("42"))], id("x"))
        ));
    }

    #[test]
    fn let_type_annotation_mismatch() {
        // let x: Boolean = 42 in x   → TypeMismatch
        check_err!(
            simple_program(let_(vec![("x", Some(ty("Boolean")), num("42"))], id("x"))),
            SemanticError::TypeMismatch { .. }
        );
    }

    #[test]
    fn let_multiple_bindings_sequential_scope() {
        // let x = 1, y = x + 1 in y   → y puede ver x
        check_ok!(simple_program(
            let_(
                vec![
                    ("x", None, num("1")),
                    ("y", None, add(id("x"), num("1"))),
                ],
                id("y"),
            )
        ));
    }

    #[test]
    fn let_body_cannot_see_outer_undefined() {
        check_err!(
            simple_program(let_(vec![("x", None, num("1"))], id("z"))),
            SemanticError::UndefinedVariable { .. }
        );
    }

    #[test]
    fn let_variable_not_visible_outside() {
        // { let x = 1 in x; x }  → x no existe en el scope exterior
        check_err!(
            simple_program(block(vec![
                let_(vec![("x", None, num("1"))], id("x")),
                id("x"),
            ])),
            SemanticError::UndefinedVariable { .. }
        );
    }

    #[test]
    fn let_string_binding_ok() {
        check_ok!(simple_program(
            let_(vec![("s", None, str_("hola"))], id("s"))
        ));
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  7. IF / ELIF / ELSE
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn if_simple_ok() {
        // if (true) 1 else 2
        check_ok!(simple_program(if_(bool_(true), num("1"), num("2"))));
    }

    #[test]
    fn if_condition_not_boolean_error() {
        // if (42) 1 else 2 → TypeMismatch
        check_err!(
            simple_program(if_(num("42"), num("1"), num("2"))),
            SemanticError::TypeMismatch { .. }
        );
    }

    #[test]
    fn if_condition_string_error() {
        check_err!(
            simple_program(if_(str_("x"), num("1"), num("2"))),
            SemanticError::TypeMismatch { .. }
        );
    }

    #[test]
    fn if_branches_same_type_ok() {
        check_ok!(simple_program(if_(bool_(true), num("1"), num("2"))));
    }

    #[test]
    fn if_branches_different_types_lca() {
        // if (true) 1 else "dos"  → LCA(Number, String) = Object, sin error
        check_ok!(simple_program(if_(bool_(true), num("1"), str_("dos"))));
    }

    #[test]
    fn if_elif_ok() {
        // if (true) 1 elif (false) 2 else 3
        check_ok!(simple_program(
            if_elif(
                bool_(true), num("1"),
                vec![(bool_(false), num("2"))],
                num("3"),
            )
        ));
    }

    #[test]
    fn if_elif_condition_not_boolean_error() {
        check_err!(
            simple_program(
                if_elif(
                    bool_(true), num("1"),
                    vec![(num("99"), num("2"))],  // ← num en condición elif
                    num("3"),
                )
            ),
            SemanticError::TypeMismatch { .. }
        );
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  8. WHILE
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn while_ok() {
        check_ok!(simple_program(while_(bool_(false), num("0"))));
    }

    #[test]
    fn while_condition_not_boolean_error() {
        check_err!(
            simple_program(while_(num("1"), num("0"))),
            SemanticError::TypeMismatch { .. }
        );
    }

    #[test]
    fn while_body_can_use_outer_scope() {
        // let x = 0 in while (true) x
        check_ok!(simple_program(
            let_(vec![("x", None, num("0"))],
                while_(bool_(true), id("x")))
        ));
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  9. FOR
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn for_over_range_ok() {
        // for (i in range(0, 10)) i
        check_ok!(simple_program(
            for_("i", call("range", vec![num("0"), num("10")]), id("i"))
        ));
    }

    #[test]
    fn for_over_vector_ok() {
        // for (x in [1, 2, 3]) x
        check_ok!(simple_program(
            for_("x", vec_explicit(vec![num("1"), num("2"), num("3")]), id("x"))
        ));
    }

    #[test]
    fn for_var_visible_only_in_body() {
        // for (i in range(0,1)) i; i  ← i no existe fuera
        check_err!(
            simple_program(block(vec![
                for_("i", call("range", vec![num("0"), num("1")]), id("i")),
                id("i"),  // i no existe aquí
            ])),
            SemanticError::UndefinedVariable { .. }
        );
    }

    #[test]
    fn for_non_iterable_error() {
        // for (x in 42) x → error
        check_err!(
            simple_program(for_("x", num("42"), id("x"))),
            SemanticError::TypeMismatch { .. }
        );
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  10. LLAMADAS A FUNCIÓN
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn call_builtin_print_ok() {
        check_ok!(simple_program(call("print", vec![num("42")])));
    }

    #[test]
    fn call_builtin_sqrt_ok() {
        check_ok!(simple_program(call("sqrt", vec![num("4")])));
    }

    #[test]
    fn call_builtin_range_ok() {
        check_ok!(simple_program(call("range", vec![num("0"), num("10")])));
    }

    #[test]
    fn call_undefined_function_error() {
        check_err!(
            simple_program(call("undefined_fn", vec![])),
            SemanticError::UndefinedFunction { .. }
        );
    }

    #[test]
    fn call_wrong_arg_count_error() {
        // sqrt espera 1, se dan 2
        check_err!(
            simple_program(call("sqrt", vec![num("1"), num("2")])),
            SemanticError::WrongArgCount { .. }
        );
    }

    #[test]
    fn call_wrong_arg_count_zero_error() {
        // print espera 1, se dan 0
        check_err!(
            simple_program(call("print", vec![])),
            SemanticError::WrongArgCount { .. }
        );
    }

    #[test]
    fn call_user_function_ok() {
        // function double(x: Number): Number => x * 2;
        // double(5)
        let p = program_with_decls(
            vec![func_decl(
                "double",
                vec![("x", Some(ty("Number")))],
                Some(ty("Number")),
                Expr::binary(BinaryOp::Mul, id("x"), num("2"), d()),
            )],
            call("double", vec![num("5")]),
        );
        check_ok!(p);
    }

    #[test]
    fn call_user_function_wrong_arg_type_error() {
        let p = program_with_decls(
            vec![func_decl(
                "inc",
                vec![("x", Some(ty("Number")))],
                Some(ty("Number")),
                add(id("x"), num("1")),
            )],
            call("inc", vec![str_("hola")]),  // ← String en vez de Number
        );
        check_err!(p, SemanticError::TypeMismatch { .. });
    }

    #[test]
    fn call_recursive_function_ok() {
        // function fact(n: Number): Number => if (n == 0) 1 else n * fact(n - 1);
        let p = program_with_decls(
            vec![func_decl(
                "fact",
                vec![("n", Some(ty("Number")))],
                Some(ty("Number")),
                if_(
                    eq_(id("n"), num("0")),
                    num("1"),
                    Expr::binary(BinaryOp::Mul, id("n"),
                        call("fact", vec![sub(id("n"), num("1"))]),
                        d()),
                ),
            )],
            call("fact", vec![num("5")]),
        );
        check_ok!(p);
    }

    #[test]
    fn function_redefinition_error() {
        let p = program_with_decls(
            vec![
                func_decl("foo", vec![], None, num("1")),
                func_decl("foo", vec![], None, num("2")),  // ← redefinición
            ],
            call("foo", vec![]),
        );
        check_err!(p, SemanticError::Redefinition { .. });
    }

    #[test]
    fn function_return_type_mismatch_error() {
        // function foo(): Boolean => 42;
        let p = program_with_decls(
            vec![func_decl("foo", vec![], Some(ty("Boolean")), num("42"))],
            call("foo", vec![]),
        );
        check_err!(p, SemanticError::TypeMismatch { .. });
    }

    #[test]
    fn function_return_type_ok() {
        let p = program_with_decls(
            vec![func_decl("get_pi", vec![], Some(ty("Number")), id("PI"))],
            call("get_pi", vec![]),
        );
        check_ok!(p);
    }

    #[test]
    fn mutually_recursive_functions_ok() {
        // function isEven(n) => if (n == 0) true else isOdd(n - 1);
        // function isOdd(n)  => if (n == 0) false else isEven(n - 1);
        let p = program_with_decls(
            vec![
                func_decl(
                    "isEven",
                    vec![("n", Some(ty("Number")))],
                    None,
                    if_(eq_(id("n"), num("0")),
                        bool_(true),
                        call("isOdd", vec![sub(id("n"), num("1"))])),
                ),
                func_decl(
                    "isOdd",
                    vec![("n", Some(ty("Number")))],
                    None,
                    if_(eq_(id("n"), num("0")),
                        bool_(false),
                        call("isEven", vec![sub(id("n"), num("1"))])),
                ),
            ],
            call("isEven", vec![num("4")]),
        );
        check_ok!(p);
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  11. DECLARACIONES DE TIPO
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn type_simple_ok() {
        // type Counter(n: Number) { value: Number = n; }
        let p = program_with_decls(
            vec![type_decl(
                "Counter",
                vec![("n", Some(ty("Number")))],
                None,
                vec![], // parent_args
                vec![attr("value", Some(ty("Number")), id("n"))],
            )],
            new_("Counter", vec![num("0")]),
        );
        check_ok!(p);
    }

    #[test]
    fn type_inherit_ok() {
        // type Animal(name: String) { name: String = name; }
        // type Dog(name: String) inherits Animal { }
        let p = program_with_decls(
            vec![
                type_decl(
                    "Animal",
                    vec![("name", Some(ty("String")))],
                    None,
                    vec![], // parent_args
                    vec![attr("name", Some(ty("String")), id("name"))],
                ),
                type_decl(
                    "Dog",
                    vec![("name", Some(ty("String")))],
                    Some("Animal"),
                    vec![id("name")], // ← argumento para el constructor del padre
                    vec![],
                ),
            ],
            new_("Dog", vec![str_("Rex")]),
        );
        check_ok!(p);
    }

    #[test]
    fn type_inherit_from_number_error() {
        let p = program_with_decls(
            vec![type_decl("MyNum", vec![], Some("Number"), vec![], vec![])],
            num("0"),
        );
        check_err!(p, SemanticError::InheritFromPrimitive { .. });
    }

    #[test]
    fn type_inherit_from_string_error() {
        let p = program_with_decls(
            vec![type_decl("MyStr", vec![], Some("String"), vec![], vec![])],
            num("0"),
        );
        check_err!(p, SemanticError::InheritFromPrimitive { .. });
    }

    #[test]
    fn type_inherit_from_boolean_error() {
        let p = program_with_decls(
            vec![type_decl("MyBool", vec![], Some("Boolean"), vec![], vec![])],
            num("0"),
        );
        check_err!(p, SemanticError::InheritFromPrimitive { .. });
    }

    #[test]
    fn type_self_in_method_ok() {
        // type Box(v: Number) {
        //     value: Number = v;
        //     get(): Number => self.value;
        // }
        let p = program_with_decls(
            vec![type_decl(
                "Box",
                vec![("v", Some(ty("Number")))],
                None,
                vec![], // parent_args
                vec![
                    attr("value", Some(ty("Number")), id("v")),
                    method("get", vec![], Some(ty("Number")), access(id("self"), "value")),
                ],
            )],
            new_("Box", vec![num("1")]),
        );
        check_ok!(p);
    }

    #[test]
    fn type_self_in_initializer_error() {
        // type Broken(v: Number) {
        //     bad = self.something;   ← self prohibido en init
        // }
        let p = program_with_decls(
            vec![type_decl(
                "Broken",
                vec![("v", Some(ty("Number")))],
                None,
                vec![], // parent_args
                vec![attr("bad", None, access(id("self"), "something"))],
            )],
            num("0"),
        );
        check_err!(p, SemanticError::SelfInInitializer { .. });
    }

    #[test]
    fn type_circular_inheritance_error() {
        // type A inherits B  /  type B inherits A
        let p = program_with_decls(
            vec![
                type_decl("A", vec![], Some("B"), vec![], vec![]),
                type_decl("B", vec![], Some("A"), vec![], vec![]),
            ],
            num("0"),
        );
        check_err!(p, SemanticError::CircularInheritance { .. });
    }

    #[test]
    fn type_new_undefined_error() {
        check_err!(
            simple_program(new_("Unicornio", vec![])),
            SemanticError::UndefinedType { .. }
        );
    }

    #[test]
    fn type_method_with_params_ok() {
        let p = program_with_decls(
            vec![type_decl(
                "Calc",
                vec![],
                None,
                vec![], // parent_args
                vec![method(
                    "add",
                    vec![("a", Some(ty("Number"))), ("b", Some(ty("Number")))],
                    Some(ty("Number")),
                    add(id("a"), id("b")),
                )],
            )],
            method_call(new_("Calc", vec![]), "add", vec![num("1"), num("2")]),
        );
        check_ok!(p);
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  12. PROTOCOLOS
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn protocol_declaration_ok() {
        let p = program_with_decls(
            vec![protocol_decl("Printable", None, vec![("show", ty("String"))])],
            num("0"),
        );
        check_ok!(p);
    }

    #[test]
    fn protocol_extends_ok() {
        let p = program_with_decls(
            vec![
                protocol_decl("Base",     None,          vec![("base_method", ty("Number"))]),
                protocol_decl("Extended", Some("Base"),  vec![("extra_method", ty("String"))]),
            ],
            num("0"),
        );
        check_ok!(p);
    }

    #[test]
    fn protocol_extends_undefined_error() {
        let p = program_with_decls(
            vec![protocol_decl("Derived", Some("NonExistent"), vec![])],
            num("0"),
        );
        check_err!(p, SemanticError::UndefinedType { .. });
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  13. ASIGNACIÓN
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn assign_variable_ok() {
        // let x = 0 in x := 42
        check_ok!(simple_program(
            let_(vec![("x", None, num("0"))],
                assign(id("x"), num("42")))
        ));
    }

    #[test]
    fn assign_type_mismatch_error() {
        // let x: Number = 0 in x := "hola"
        check_err!(
            simple_program(
                let_(vec![("x", Some(ty("Number")), num("0"))],
                    assign(id("x"), str_("hola")))
            ),
            SemanticError::TypeMismatch { .. }
        );
    }

    #[test]
    fn assign_to_self_error() {
        // En un método: self := algo → error
        let p = program_with_decls(
            vec![type_decl(
                "T",
                vec![],
                None,
                vec![], // parent_args
                vec![method(
                    "bad",
                    vec![],
                    None,
                    assign(id("self"), num("1")),  // ← self := 1
                )],
            )],
            num("0"),
        );
        check_err!(p, SemanticError::SelfAssignment { .. });
    }

    #[test]
    fn assign_invalid_lvalue_error() {
        // 42 := 1 → InvalidLValue
        check_err!(
            simple_program(assign(num("42"), num("1"))),
            SemanticError::InvalidLValue { .. }
        );
    }

    #[test]
    fn assign_invalid_lvalue_call_error() {
        // foo() := 1 → InvalidLValue
        check_err!(
            simple_program(assign(call("print", vec![num("1")]), num("1"))),
            SemanticError::InvalidLValue { .. }
        );
    }

    #[test]
    fn assign_compound_ok() {
        // let x = 1 in x += 2
        check_ok!(simple_program(
            let_(vec![("x", None, num("1"))],
                Expr::assign(AssignOp::PlusAssign, id("x"), num("2"), d()))
        ));
    }

    #[test]
    fn assign_compound_wrong_type_error() {
        // let s = "hola" in s += 1  → error, += requiere Number
        check_err!(
            simple_program(
                let_(vec![("s", None, str_("hola"))],
                    Expr::assign(AssignOp::PlusAssign, id("s"), num("1"), d()))
            ),
            SemanticError::InvalidOperandType { .. }
        );
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  14. IS / AS
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn is_expr_ok() {
        check_ok!(simple_program(is_expr(num("42"), "Number")));
    }

    #[test]
    fn is_undefined_type_error() {
        check_err!(
            simple_program(is_expr(num("42"), "Fantasma")),
            SemanticError::UndefinedType { .. }
        );
    }

    #[test]
    fn as_upcast_ok() {
        // (new Dog()) as Animal
        let p = program_with_decls(
            vec![
                type_decl("Animal", vec![], None, vec![], vec![]),
                type_decl("Dog",    vec![], Some("Animal"), vec![], vec![]),
            ],
            as_expr(new_("Dog", vec![]), "Animal"),
        );
        check_ok!(p);
    }

    #[test]
    fn as_downcast_ok() {
        // (new Animal()) as Dog  — semánticamente válido, falla en runtime si no aplica
        let p = program_with_decls(
            vec![
                type_decl("Animal", vec![], None, vec![], vec![]),
                type_decl("Dog",    vec![], Some("Animal"), vec![], vec![]),
            ],
            as_expr(new_("Animal", vec![]), "Dog"),
        );
        check_ok!(p);
    }

    #[test]
    fn as_unrelated_types_error() {
        // (new Cat()) as Dog  — sin relación de herencia
        let p = program_with_decls(
            vec![
                type_decl("Cat", vec![], None, vec![], vec![]),
                type_decl("Dog", vec![], None, vec![], vec![]),
            ],
            as_expr(new_("Cat", vec![]), "Dog"),
        );
        check_err!(p, SemanticError::InvalidCast { .. });
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  15. VECTORES
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn vector_explicit_homogeneous_ok() {
        check_ok!(simple_program(vec_explicit(vec![num("1"), num("2"), num("3")])));
    }

    #[test]
    fn vector_explicit_empty_ok() {
        check_ok!(simple_program(vec_explicit(vec![])));
    }

    #[test]
    fn vector_explicit_mixed_types_ok() {
        // [1, "dos"] → Vector[Object] via LCA
        check_ok!(simple_program(vec_explicit(vec![num("1"), str_("dos")])));
    }

    #[test]
    fn vector_generator_ok() {
        // [x * 2 | x in range(0, 5)]
        check_ok!(simple_program(
            vec_gen(
                Expr::binary(BinaryOp::Mul, id("x"), num("2"), d()),
                "x",
                call("range", vec![num("0"), num("5")]),
            )
        ));
    }

    #[test]
    fn vector_generator_var_scoped_ok() {
        // La variable del generador solo existe dentro del body
        check_err!(
            simple_program(block(vec![
                vec_gen(id("x"), "x", call("range", vec![num("0"), num("1")])),
                id("x"),  // x no existe aquí
            ])),
            SemanticError::UndefinedVariable { .. }
        );
    }

    #[test]
    fn vector_index_ok() {
        // let v = [1, 2, 3] in v[0]
        check_ok!(simple_program(
            let_(
                vec![("v", None, vec_explicit(vec![num("1"), num("2"), num("3")]))],
                Expr::Index(Box::new(
                    crate::parser::ast::IndexExpr::new(id("v"), num("0"), d())
                ))
            )
        ));
    }

    #[test]
    fn vector_index_non_number_error() {
        // let v = [1] in v["cero"]
        check_err!(
            simple_program(
                let_(
                    vec![("v", None, vec_explicit(vec![num("1")]))],
                    Expr::Index(Box::new(
                        crate::parser::ast::IndexExpr::new(id("v"), str_("cero"), d())
                    ))
                )
            ),
            SemanticError::TypeMismatch { .. }
        );
    }

    #[test]
    fn index_non_vector_error() {
        check_err!(
            simple_program(
                let_(
                    vec![("x", None, num("5"))],
                    Expr::Index(Box::new(
                        crate::parser::ast::IndexExpr::new(id("x"), num("0"), d())
                    ))
                )
            ),
            SemanticError::TypeMismatch { .. }
        );
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  16. ACCESOS Y MÉTODOS
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn method_call_ok() {
        // type Greeter { greet(): String => "hola"; }
        // new Greeter().greet()
        let p = program_with_decls(
            vec![type_decl(
                "Greeter",
                vec![],
                None,
                vec![], // parent_args
                vec![method("greet", vec![], Some(ty("String")), str_("hola"))],
            )],
            method_call(new_("Greeter", vec![]), "greet", vec![]),
        );
        check_ok!(p);
    }

    #[test]
    fn method_not_found_error() {
        let p = program_with_decls(
            vec![type_decl("Empty", vec![], None, vec![], vec![])],
            method_call(new_("Empty", vec![]), "nonexistent", vec![]),
        );
        check_err!(p, SemanticError::MethodNotFound { .. });
    }

    #[test]
    fn attribute_access_not_found_error() {
        let p = program_with_decls(
            vec![type_decl("Point", vec![], None, vec![], vec![])],
            access(new_("Point", vec![]), "z"),
        );
        check_err!(p, SemanticError::AttributeNotFound { .. });
    }

    #[test]
    fn inherited_method_accessible_ok() {
        // type A { foo(): Number => 1; }
        // type B inherits A { }
        // new B().foo()   ← hereda foo de A
        let p = program_with_decls(
            vec![
                type_decl(
                    "A", vec![], None, vec![],
                    vec![method("foo", vec![], Some(ty("Number")), num("1"))],
                ),
                type_decl("B", vec![], Some("A"), vec![], vec![]),
            ],
            method_call(new_("B", vec![]), "foo", vec![]),
        );
        check_ok!(p);
    }

    #[test]
    fn builtin_string_size_ok() {
        // "hola".size()
        check_ok!(simple_program(method_call(str_("hola"), "size", vec![])));
    }

    #[test]
    fn builtin_tostring_ok() {
        check_ok!(simple_program(method_call(num("42"), "toString", vec![])));
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  17. BASE
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn base_in_method_ok() {
        // type A { val(): Number => 1; }
        // type B inherits A { val(): Number => base() + 1; }
        let p = program_with_decls(
            vec![
                type_decl(
                    "A", vec![], None, vec![],
                    vec![method("val", vec![], Some(ty("Number")), num("1"))],
                ),
                type_decl(
                    "B", vec![], Some("A"), vec![],
                    vec![method(
                        "val", vec![], Some(ty("Number")),
                        add(Expr::Base(d()), num("1")),
                    )],
                ),
            ],
            method_call(new_("B", vec![]), "val", vec![]),
        );
        // base() existe → no debe haber UndefinedVariable para "base"
        let errors = match analyze(&p) {
            Ok(()) => vec![],
            Err(e) => e,
        };
        assert!(!errors.iter().any(|e| {
            if let SemanticError::UndefinedVariable { name, .. } = e {
                name == "base"
            } else {
                false
            }
        }), "No se esperaba UndefinedVariable para 'base', pero apareció");
    }

    #[test]
    fn base_outside_type_error() {
        // Usar base() fuera de un tipo
        check_err!(
            simple_program(Expr::Base(d())),
            SemanticError::UndefinedVariable { .. }
        );
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  18. PROPAGACIÓN DE Never (sin cascada de errores)
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn never_does_not_cascade_in_binary() {
        // undefined_var + 1  → solo 1 error (UndefinedVariable), no InvalidBinaryTypes
        let errors = match analyze(&simple_program(add(id("undefined_var"), num("1")))) {
            Ok(())      => vec![],
            Err(errors) => errors,
        };
        assert_eq!(
            errors.iter().filter(|e| matches!(e, SemanticError::UndefinedVariable { .. })).count(),
            1,
            "Debe haber exactamente 1 UndefinedVariable"
        );
        assert!(
            !errors.iter().any(|e| matches!(e, SemanticError::InvalidBinaryTypes { .. })),
            "No debe haber InvalidBinaryTypes cuando un operando es Never"
        );
    }

    #[test]
    fn never_does_not_cascade_in_if() {
        // if (undefined) 1 else 2  → solo 1 error, no TypeMismatch en condición
        let count = error_count!(simple_program(
            if_(id("undefined_cond"), num("1"), num("2"))
        ));
        assert_eq!(count, 1, "Solo debe haber 1 error (UndefinedVariable)");
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  19. PROGRAMAS COMPLETOS (integración)
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn complete_fibonacci_ok() {
        // function fib(n: Number): Number =>
        //   if (n == 0) 0
        //   elif (n == 1) 1
        //   else fib(n-1) + fib(n-2);
        // print(fib(10))
        let p = program_with_decls(
            vec![func_decl(
                "fib",
                vec![("n", Some(ty("Number")))],
                Some(ty("Number")),
                if_elif(
                    eq_(id("n"), num("0")),
                    num("0"),
                    vec![(eq_(id("n"), num("1")), num("1"))],
                    add(
                        call("fib", vec![sub(id("n"), num("1"))]),
                        call("fib", vec![sub(id("n"), num("2"))]),
                    ),
                ),
            )],
            call("print", vec![call("fib", vec![num("10")])]),
        );
        check_ok!(p);
    }

    #[test]
    fn complete_point_type_ok() {
        // type Point(x: Number, y: Number) {
        //     x: Number = x;
        //     y: Number = y;
        //     dist(): Number => sqrt(self.x * self.x + self.y * self.y);
        // }
        // let p = new Point(3, 4) in p.dist()
        let p = program_with_decls(
            vec![type_decl(
                "Point",
                vec![("x", Some(ty("Number"))), ("y", Some(ty("Number")))],
                None,
                vec![], // parent_args
                vec![
                    attr("x", Some(ty("Number")), id("x")),
                    attr("y", Some(ty("Number")), id("y")),
                    method(
                        "dist", vec![], Some(ty("Number")),
                        call("sqrt", vec![
                            add(
                                Expr::binary(BinaryOp::Mul,
                                    access(id("self"), "x"),
                                    access(id("self"), "x"), d()),
                                Expr::binary(BinaryOp::Mul,
                                    access(id("self"), "y"),
                                    access(id("self"), "y"), d()),
                            )
                        ]),
                    ),
                ],
            )],
            let_(
                vec![("p", None, new_("Point", vec![num("3"), num("4")]))],
                method_call(id("p"), "dist", vec![]),
            ),
        );
        check_ok!(p);
    }

    #[test]
    fn complete_vector_sum_ok() {
        // let v = [1, 2, 3, 4, 5] in
        // let total = 0 in
        // { for (x in v) total := total + x; total }
        let p = simple_program(
            let_(
                vec![("v", None, vec_explicit(vec![num("1"), num("2"), num("3")]))],
                let_(
                    vec![("total", None, num("0"))],
                    block(vec![
                        for_("x", id("v"), assign(id("total"), add(id("total"), id("x")))),
                        id("total"),
                    ]),
                ),
            )
        );
        check_ok!(p);
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  20. ARIDAD DEL CONSTRUCTOR (mejora 2)
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn constructor_correct_arity_ok() {
        // type Point(x: Number, y: Number) { ... }
        // new Point(1, 2) → ok
        let p = program_with_decls(
            vec![type_decl(
                "Point",
                vec![("x", Some(ty("Number"))), ("y", Some(ty("Number")))],
                None,
                vec![],
                vec![
                    attr("x", Some(ty("Number")), id("x")),
                    attr("y", Some(ty("Number")), id("y")),
                ],
            )],
            new_("Point", vec![num("1"), num("2")]),
        );
        check_ok!(p);
    }

    #[test]
    fn constructor_too_few_args_error() {
        // new Point(1)  cuando Point(x, y) → WrongArgCount
        let p = program_with_decls(
            vec![type_decl(
                "Point",
                vec![("x", Some(ty("Number"))), ("y", Some(ty("Number")))],
                None,
                vec![],
                vec![],
            )],
            new_("Point", vec![num("1")]),
        );
        check_err!(p, SemanticError::WrongArgCount { .. });
    }

    #[test]
    fn constructor_too_many_args_error() {
        // new Point(1, 2, 3) cuando Point(x, y) → WrongArgCount
        let p = program_with_decls(
            vec![type_decl(
                "Point",
                vec![("x", Some(ty("Number"))), ("y", Some(ty("Number")))],
                None,
                vec![],
                vec![],
            )],
            new_("Point", vec![num("1"), num("2"), num("3")]),
        );
        check_err!(p, SemanticError::WrongArgCount { .. });
    }

    #[test]
    fn constructor_wrong_arg_type_error() {
        // type Point(x: Number, y: Number)
        // new Point(1, "dos") → TypeMismatch
        let p = program_with_decls(
            vec![type_decl(
                "Point",
                vec![("x", Some(ty("Number"))), ("y", Some(ty("Number")))],
                None,
                vec![],
                vec![],
            )],
            new_("Point", vec![num("1"), str_("dos")]),
        );
        check_err!(p, SemanticError::TypeMismatch { .. });
    }

    #[test]
    fn constructor_no_args_ok() {
        // type Empty() { }   new Empty() → ok
        let p = program_with_decls(
            vec![type_decl("Empty", vec![], None, vec![], vec![])],
            new_("Empty", vec![]),
        );
        check_ok!(p);
    }

    #[test]
    fn constructor_no_args_with_args_error() {
        // type Empty()   new Empty(1) → error
        let p = program_with_decls(
            vec![type_decl("Empty", vec![], None, vec![], vec![])],
            new_("Empty", vec![num("1")]),
        );
        check_err!(p, SemanticError::WrongArgCount { .. });
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  21. OVERRIDE ESTRICTO (mejora 4)
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn override_same_signature_ok() {
        // type A { foo(x: Number): Number => x; }
        // type B inherits A { foo(x: Number): Number => x + 1; }  → ok
        let p = program_with_decls(
            vec![
                type_decl(
                    "A", vec![], None, vec![],
                    vec![method("foo", vec![("x", Some(ty("Number")))], Some(ty("Number")), id("x"))],
                ),
                type_decl(
                    "B", vec![], Some("A"), vec![],
                    vec![method("foo", vec![("x", Some(ty("Number")))], Some(ty("Number")),
                        add(id("x"), num("1")))],
                ),
            ],
            num("0"),
        );
        check_ok!(p);
    }

    #[test]
    fn override_different_param_count_error() {
        // type A { foo(x: Number): Number => x; }
        // type B inherits A { foo(x: Number, y: Number): Number => x; }  → OverrideMismatch
        let p = program_with_decls(
            vec![
                type_decl(
                    "A", vec![], None, vec![],
                    vec![method("foo", vec![("x", Some(ty("Number")))], Some(ty("Number")), id("x"))],
                ),
                type_decl(
                    "B", vec![], Some("A"), vec![],
                    vec![method(
                        "foo",
                        vec![("x", Some(ty("Number"))), ("y", Some(ty("Number")))],
                        Some(ty("Number")),
                        id("x"),
                    )],
                ),
            ],
            num("0"),
        );
        check_err!(p, SemanticError::OverrideMismatch { .. });
    }

    #[test]
    fn override_different_param_type_error() {
        // type A { foo(x: Number): Number => x; }
        // type B inherits A { foo(x: String): Number => 0; }  → OverrideMismatch
        let p = program_with_decls(
            vec![
                type_decl(
                    "A", vec![], None, vec![],
                    vec![method("foo", vec![("x", Some(ty("Number")))], Some(ty("Number")), id("x"))],
                ),
                type_decl(
                    "B", vec![], Some("A"), vec![],
                    vec![method("foo", vec![("x", Some(ty("String")))], Some(ty("Number")), num("0"))],
                ),
            ],
            num("0"),
        );
        check_err!(p, SemanticError::OverrideMismatch { .. });
    }

    #[test]
    fn override_incompatible_return_type_error() {
        // type A { foo(): Number => 1; }
        // type B inherits A { foo(): String => "x"; }  → OverrideMismatch
        let p = program_with_decls(
            vec![
                type_decl(
                    "A", vec![], None, vec![],
                    vec![method("foo", vec![], Some(ty("Number")), num("1"))],
                ),
                type_decl(
                    "B", vec![], Some("A"), vec![],
                    vec![method("foo", vec![], Some(ty("String")), str_("x"))],
                ),
            ],
            num("0"),
        );
        check_err!(p, SemanticError::OverrideMismatch { .. });
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  22. BASE() COMO LLAMADA (mejora 6)
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn base_call_in_constructor_ok() {
        // type Animal(name: String) { name: String = name; }
        // type Dog(name: String) inherits Animal { ... base(name) ... }
        let p = program_with_decls(
            vec![
                type_decl(
                    "Animal",
                    vec![("name", Some(ty("String")))],
                    None,
                    vec![],
                    vec![attr("name", Some(ty("String")), id("name"))],
                ),
                {
                    let ps = vec![crate::parser::ast::Param::new("name", Some(ty("String")), d())];
                    let par = Some(ty("Animal"));
                    let par_args = vec![id("name")];
                    let members = vec![method(
                        "init", vec![], None,
                        Expr::Call(Box::new(crate::parser::ast::CallExpr::new(
                            Expr::Base(d()),
                            vec![id("name")],
                            d(),
                        ))),
                    )];
                    Decl::Type(TypeDecl::new("Dog", ps, par, par_args, members, d()))
                },
            ],
            new_("Dog", vec![str_("Rex")]),
        );
        check_ok!(p);
    }

    #[test]
    fn base_call_wrong_arg_count_error() {
        // type Animal(name: String) { ... }
        // type Dog(name: String) inherits Animal { method uses base() — sin args cuando debe ser 1 }
        let p = program_with_decls(
            vec![
                type_decl(
                    "Animal",
                    vec![("name", Some(ty("String")))],
                    None,
                    vec![],
                    vec![attr("name", Some(ty("String")), id("name"))],
                ),
                {
                    let ps = vec![crate::parser::ast::Param::new("name", Some(ty("String")), d())];
                    let par = Some(ty("Animal"));
                    let par_args = vec![];
                    let members = vec![method(
                        "bad_init", vec![], None,
                        Expr::Call(Box::new(crate::parser::ast::CallExpr::new(
                            Expr::Base(d()),
                            vec![],
                            d(),
                        ))),
                    )];
                    Decl::Type(TypeDecl::new("Dog", ps, par, par_args, members, d()))
                },
            ],
            num("0"),
        );
        check_err!(p, SemanticError::WrongArgCount { .. });
    }

    #[test]
    fn base_call_outside_type_error() {
        // base() fuera de un tipo → UndefinedVariable
        check_err!(
            simple_program(Expr::Call(Box::new(crate::parser::ast::CallExpr::new(
                Expr::Base(d()),
                vec![],
                d(),
            )))),
            SemanticError::UndefinedVariable { .. }
        );
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  23. VECTORES ANOTADOS (mejora 7)
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn vector_annotation_correct_ok() {
        // let v: Number[] = [1, 2, 3] in v
        check_ok!(simple_program(
            let_(
                vec![("v", Some(TypeName::vector("Number", d())), vec_explicit(vec![num("1"), num("2")]))],
                id("v"),
            )
        ));
    }

    #[test]
    fn vector_annotation_covariant_ok() {
        // let v: Object[] = [1, 2, 3] in v   → Number[] conforma con Object[]
        check_ok!(simple_program(
            let_(
                vec![("v", Some(TypeName::vector("Object", d())), vec_explicit(vec![num("1"), num("2")]))],
                id("v"),
            )
        ));
    }

    #[test]
    fn vector_annotation_type_mismatch_error() {
        // let v: Number[] = ["a", "b"] in v → TypeMismatch
        check_err!(
            simple_program(
                let_(
                    vec![("v", Some(TypeName::vector("Number", d())),
                          vec_explicit(vec![str_("a"), str_("b")]))],
                    id("v"),
                )
            ),
            SemanticError::TypeMismatch { .. }
        );
    }

    #[test]
    fn vector_annotation_bool_mismatch_error() {
        // let v: Boolean[] = [1, 2] → TypeMismatch
        check_err!(
            simple_program(
                let_(
                    vec![("v", Some(TypeName::vector("Boolean", d())),
                          vec_explicit(vec![num("1"), num("2")]))],
                    id("v"),
                )
            ),
            SemanticError::TypeMismatch { .. }
        );
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  24. INFERENCIA DE RETORNO (mejora 1)
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn infer_return_type_used_in_arithmetic_ok() {
        // function double(x: Number) => x * 2;   ← sin anotación de retorno
        // let r: Number = double(5) in r          ← el tipo inferido debe ser Number
        let p = program_with_decls(
            vec![func_decl(
                "double",
                vec![("x", Some(ty("Number")))],
                None,  // ← sin anotación de retorno
                Expr::binary(BinaryOp::Mul, id("x"), num("2"), d()),
            )],
            let_(
                vec![("r", Some(ty("Number")), call("double", vec![num("5")]))],
                id("r"),
            ),
        );
        check_ok!(p);
    }

    #[test]
    fn infer_return_type_mismatch_error() {
        // function get_name() => "hulk";  ← infiere String
        // let n: Number = get_name() in n  → TypeMismatch
        let p = program_with_decls(
            vec![func_decl("get_name", vec![], None, str_("hulk"))],
            let_(
                vec![("n", Some(ty("Number")), call("get_name", vec![]))],
                id("n"),
            ),
        );
        check_err!(p, SemanticError::TypeMismatch { .. });
    }

    #[test]
    fn infer_return_type_propagates_to_call_ok() {
        // function square(x: Number) => x ^ 2;
        // square(3) + 1   ← debería funcionar porque square infiere Number
        let p = program_with_decls(
            vec![func_decl(
                "square",
                vec![("x", Some(ty("Number")))],
                None,
                Expr::binary(BinaryOp::Power, id("x"), num("2"), d()),
            )],
            add(call("square", vec![num("3")]), num("1")),
        );
        check_ok!(p);
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  25. CONFORMANCE DE PROTOCOLOS EN CONTEXTO (mejora 3)
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn protocol_let_binding_conforming_ok() {
        // protocol Printable { show(): String; }
        // type Dog { show(): String => "woof"; }
        // let x: Printable = new Dog() in x
        let p = program_with_decls(
            vec![
                protocol_decl("Printable", None, vec![("show", ty("String"))]),
                type_decl(
                    "Dog", vec![], None, vec![],
                    vec![method("show", vec![], Some(ty("String")), str_("woof"))],
                ),
            ],
            let_(
                vec![("x", Some(ty("Printable")), new_("Dog", vec![]))],
                id("x"),
            ),
        );
        check_ok!(p);
    }

    #[test]
    fn protocol_let_binding_not_conforming_error() {
        // protocol Printable { show(): String; }
        // type Cat { }   ← no tiene show()
        // let x: Printable = new Cat() in x → ProtocolNotConformed
        let p = program_with_decls(
            vec![
                protocol_decl("Printable", None, vec![("show", ty("String"))]),
                type_decl("Cat", vec![], None, vec![], vec![]),
            ],
            let_(
                vec![("x", Some(ty("Printable")), new_("Cat", vec![]))],
                id("x"),
            ),
        );
        check_err!(p, SemanticError::ProtocolNotConformed { .. });
    }

    #[test]
    fn protocol_function_param_conforming_ok() {
        // protocol Drawable { draw(): String; }
        // function render(d: Drawable): String => d.draw();
        // type Circle { draw(): String => "circle"; }
        // render(new Circle())
        let p = program_with_decls(
            vec![
                protocol_decl("Drawable", None, vec![("draw", ty("String"))]),
                func_decl(
                    "render",
                    vec![("d", Some(ty("Drawable")))],
                    Some(ty("String")),
                    method_call(id("d"), "draw", vec![]),
                ),
                type_decl(
                    "Circle", vec![], None, vec![],
                    vec![method("draw", vec![], Some(ty("String")), str_("circle"))],
                ),
            ],
            call("render", vec![new_("Circle", vec![])]),
        );
        check_ok!(p);
    }

    #[test]
    fn protocol_function_param_not_conforming_error() {
        // protocol Drawable { draw(): String; }
        // function render(d: Drawable): String => d.draw();
        // type Square { }   ← no tiene draw()
        // render(new Square()) → ProtocolNotConformed
        let p = program_with_decls(
            vec![
                protocol_decl("Drawable", None, vec![("draw", ty("String"))]),
                func_decl(
                    "render",
                    vec![("d", Some(ty("Drawable")))],
                    Some(ty("String")),
                    method_call(id("d"), "draw", vec![]),
                ),
                type_decl("Square", vec![], None, vec![], vec![]),
            ],
            call("render", vec![new_("Square", vec![])]),
        );
        check_err!(p, SemanticError::ProtocolNotConformed { .. });
    }

    #[test]
    fn protocol_inherited_methods_count_ok() {
        // protocol Greetable { greet(): String; }
        // type Base { greet(): String => "hi"; }
        // type Child inherits Base { }   ← hereda greet()
        // let x: Greetable = new Child() in x  → ok
        let p = program_with_decls(
            vec![
                protocol_decl("Greetable", None, vec![("greet", ty("String"))]),
                type_decl(
                    "Base", vec![], None, vec![],
                    vec![method("greet", vec![], Some(ty("String")), str_("hi"))],
                ),
                type_decl("Child", vec![], Some("Base"), vec![], vec![]),
            ],
            let_(
                vec![("x", Some(ty("Greetable")), new_("Child", vec![]))],
                id("x"),
            ),
        );
        check_ok!(p);
    }

    #[test]
    fn protocol_parent_type_conforms_child_also_conforms_ok() {
        // protocol Flyable { fly(): Boolean; }
        // type Bird { fly(): Boolean => true; }
        // type Eagle inherits Bird { }  ← padre conforma → hijo también
        // let x: Flyable = new Eagle() in x  → ok
        let p = program_with_decls(
            vec![
                protocol_decl("Flyable", None, vec![("fly", ty("Boolean"))]),
                type_decl(
                    "Bird", vec![], None, vec![],
                    vec![method("fly", vec![], Some(ty("Boolean")), bool_(true))],
                ),
                type_decl("Eagle", vec![], Some("Bird"), vec![], vec![]),
            ],
            let_(
                vec![("x", Some(ty("Flyable")), new_("Eagle", vec![]))],
                id("x"),
            ),
        );
        check_ok!(p);
    }

    #[test]
    fn protocol_covariant_return_ok() {
        // protocol Provider { get(): Object; }
        // type NumProvider { get(): Number => 42; }  ← Number es subtipo de Object (covariante OK)
        // let p: Provider = new NumProvider() in p
        let p = program_with_decls(
            vec![
                protocol_decl("Provider", None, vec![("get", ty("Object"))]),
                type_decl(
                    "NumProvider", vec![], None, vec![],
                    vec![method("get", vec![], Some(ty("Number")), num("42"))],
                ),
            ],
            let_(
                vec![("p", Some(ty("Provider")), new_("NumProvider", vec![]))],
                id("p"),
            ),
        );
        check_ok!(p);
    }

    #[test]
    fn protocol_invariant_return_mismatch_error() {
        // protocol Named { name(): Number; }
        // type Foo { name(): String => "foo"; }  ← String NO conforma Number
        // let x: Named = new Foo() in x  → ProtocolNotConformed
        let p = program_with_decls(
            vec![
                protocol_decl("Named", None, vec![("name", ty("Number"))]),
                type_decl(
                    "Foo", vec![], None, vec![],
                    vec![method("name", vec![], Some(ty("String")), str_("foo"))],
                ),
            ],
            let_(
                vec![("x", Some(ty("Named")), new_("Foo", vec![]))],
                id("x"),
            ),
        );
        check_err!(p, SemanticError::ProtocolNotConformed { .. });
    }

    #[test]
    fn protocol_extends_chain_ok() {
        // protocol Base    { base_method(): Number; }
        // protocol Derived extends Base { extra(): String; }
        // type Impl { base_method(): Number => 1; extra(): String => "x"; }
        // let x: Derived = new Impl() in x  → ok
        let p = program_with_decls(
            vec![
                protocol_decl("Base",    None,          vec![("base_method", ty("Number"))]),
                protocol_decl("Derived", Some("Base"),  vec![("extra", ty("String"))]),
                type_decl(
                    "Impl", vec![], None, vec![],
                    vec![
                        method("base_method", vec![], Some(ty("Number")), num("1")),
                        method("extra",        vec![], Some(ty("String")), str_("x")),
                    ],
                ),
            ],
            let_(
                vec![("x", Some(ty("Derived")), new_("Impl", vec![]))],
                id("x"),
            ),
        );
        check_ok!(p);
    }

    #[test]
    fn protocol_extends_missing_parent_method_error() {
        // protocol Base    { base_method(): Number; }
        // protocol Derived extends Base { extra(): String; }
        // type Partial { extra(): String => "x"; }  ← falta base_method
        // let x: Derived = new Partial() in x  → ProtocolNotConformed
        let p = program_with_decls(
            vec![
                protocol_decl("Base",    None,          vec![("base_method", ty("Number"))]),
                protocol_decl("Derived", Some("Base"),  vec![("extra", ty("String"))]),
                type_decl(
                    "Partial", vec![], None, vec![],
                    vec![method("extra", vec![], Some(ty("String")), str_("x"))],
                ),
            ],
            let_(
                vec![("x", Some(ty("Derived")), new_("Partial", vec![]))],
                id("x"),
            ),
        );
        check_err!(p, SemanticError::ProtocolNotConformed { .. });
    }

    #[test]
    fn protocol_as_return_type_ok() {
        // protocol Hashable { hash(): Number; }
        // type Key { hash(): Number => 99; }
        // function make_hashable(): Hashable => new Key();
        let p = program_with_decls(
            vec![
                protocol_decl("Hashable", None, vec![("hash", ty("Number"))]),
                type_decl(
                    "Key", vec![], None, vec![],
                    vec![method("hash", vec![], Some(ty("Number")), num("99"))],
                ),
                func_decl("make_hashable", vec![], Some(ty("Hashable")), new_("Key", vec![])),
            ],
            call("make_hashable", vec![]),
        );
        check_ok!(p);
    }

    #[test]
    fn protocol_error_message_includes_missing_method() {
        // protocol Serializable { serialize(): String; deserialize(): Object; }
        // type Broken { serialize(): String => "ok"; }  ← falta deserialize
        // let x: Serializable = new Broken() in x
        let p = program_with_decls(
            vec![
                protocol_decl("Serializable", None, vec![
                    ("serialize",   ty("String")),
                    ("deserialize", ty("Object")),
                ]),
                type_decl(
                    "Broken", vec![], None, vec![],
                    vec![method("serialize", vec![], Some(ty("String")), str_("ok"))],
                ),
            ],
            let_(
                vec![("x", Some(ty("Serializable")), new_("Broken", vec![]))],
                id("x"),
            ),
        );
        let errors = match analyze(&p) {
            Ok(())      => vec![],
            Err(errors) => errors,
        };
        // Debe haber ProtocolNotConformed y el mensaje debe mencionar "deserialize"
        let found = errors.iter().find(|e| matches!(e, SemanticError::ProtocolNotConformed { .. }));
        assert!(found.is_some(), "Se esperaba ProtocolNotConformed");
        if let Some(SemanticError::ProtocolNotConformed { missing, .. }) = found {
            assert!(
                missing.contains("deserialize"),
                "El error debe mencionar 'deserialize', mencionó: '{}'", missing
            );
        }
    }
    // ═════════════════════════════════════════════════════════════════════════
    //  Sigue el test original de múltiples errores
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn complete_multiple_errors_detected() {
        // Un programa con varios errores: se detectan todos
        let p = program_with_decls(
            vec![
                func_decl("foo", vec![], None, id("undefined_in_foo")),
                func_decl("foo", vec![], None, num("1")),  // redefinición
            ],
            add(str_("x"), bool_(true)),  // InvalidBinaryTypes
        );
        let errors = match analyze(&p) {
            Ok(())      => vec![],
            Err(errors) => errors,
        };
        assert!(
            errors.len() >= 3,
            "Se esperaban al menos 3 errores, hubo {}:\n{}",
            errors.len(),
            errors.iter().map(|e| format!("  • {}", e)).collect::<Vec<_>>().join("\n")
        );
    }

    // ═════════════════════════════════════════════════════════════════════════
    //  26. PARENT_ARGS — argumentos al constructor del padre (del PDF)
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn parent_args_correct_ok() {
        // type Shape(color: String) { color: String = color; }
        // type Circle(r: Number) inherits Shape("red") { r: Number = r; }
        // new Circle(5)
        let circle = {
            let ps = vec![crate::parser::ast::Param::new("r", Some(ty("Number")), d())];
            let par = Some(ty("Shape"));
            let par_args = vec![str_("red")];
            let members = vec![attr("r", Some(ty("Number")), id("r"))];
            Decl::Type(crate::parser::ast::TypeDecl::new(
                "Circle", ps, par, par_args, members, d(),
            ))
        };
        let p = program_with_decls(
            vec![
                type_decl(
                    "Shape",
                    vec![("color", Some(ty("String")))],
                    None,
                    vec![],
                    vec![attr("color", Some(ty("String")), id("color"))],
                ),
                circle,
            ],
            new_("Circle", vec![num("5")]),
        );
        check_ok!(p);
    }

    #[test]
    fn parent_args_wrong_arity_error() {
        // type Animal(name: String, age: Number) { ... }
        // type Dog(name: String) inherits Animal(name)  ← falta age
        let dog = {
            let ps = vec![crate::parser::ast::Param::new("name", Some(ty("String")), d())];
            let par = Some(ty("Animal"));
            let par_args = vec![id("name")]; // solo 1 arg, Animal espera 2
            Decl::Type(crate::parser::ast::TypeDecl::new(
                "Dog", ps, par, par_args, vec![], d(),
            ))
        };
        let p = program_with_decls(
            vec![
                type_decl(
                    "Animal",
                    vec![("name", Some(ty("String"))), ("age", Some(ty("Number")))],
                    None,
                    vec![],
                    vec![],
                ),
                dog,
            ],
            num("0"),
        );
        check_err!(p, SemanticError::WrongArgCount { .. });
    }

    #[test]
    fn parent_args_wrong_type_error() {
        // type Base(x: Number) { }
        // type Child(y: Number) inherits Base("wrong_type")  ← String en vez de Number
        let child = {
            let ps = vec![crate::parser::ast::Param::new("y", Some(ty("Number")), d())];
            let par = Some(ty("Base"));
            let par_args = vec![str_("wrong")]; // String en vez de Number
            Decl::Type(crate::parser::ast::TypeDecl::new(
                "Child", ps, par, par_args, vec![], d(),
            ))
        };
        let p = program_with_decls(
            vec![
                type_decl("Base", vec![("x", Some(ty("Number")))], None, vec![], vec![]),
                child,
            ],
            num("0"),
        );
        check_err!(p, SemanticError::TypeMismatch { .. });
    }

    #[test]
    fn parent_args_uses_own_constructor_params_ok() {
        // type Point(x: Number, y: Number) { ... }
        // type Point3D(x: Number, y: Number, z: Number) inherits Point(x, y)
        //                                                              ^^^^ params propios
        let p3d = {
            let ps = vec![
                crate::parser::ast::Param::new("x", Some(ty("Number")), d()),
                crate::parser::ast::Param::new("y", Some(ty("Number")), d()),
                crate::parser::ast::Param::new("z", Some(ty("Number")), d()),
            ];
            let par = Some(ty("Point"));
            let par_args = vec![id("x"), id("y")]; // usa params propios
            Decl::Type(crate::parser::ast::TypeDecl::new(
                "Point3D", ps, par, par_args, vec![], d(),
            ))
        };
        let p = program_with_decls(
            vec![
                type_decl(
                    "Point",
                    vec![("x", Some(ty("Number"))), ("y", Some(ty("Number")))],
                    None,
                    vec![],
                    vec![
                        attr("x", Some(ty("Number")), id("x")),
                        attr("y", Some(ty("Number")), id("y")),
                    ],
                ),
                p3d,
            ],
            new_("Point3D", vec![num("1"), num("2"), num("3")]),
        );
        check_ok!(p);
    }

    #[test]
    fn parent_no_args_constructor_needs_none_ok() {
        // type Base() { }   type Child() inherits Base { }  ← Base no tiene args → ok
        let p = program_with_decls(
            vec![
                type_decl("Base",  vec![], None, vec![], vec![]),
                type_decl("Child", vec![], Some("Base"), vec![], vec![]),
            ],
            new_("Child", vec![]),
        );
        check_ok!(p);
    }

    #[test]
    fn parent_with_args_no_parent_args_error() {
        // type Base(x: Number) { }
        // type Child() inherits Base { }  ← omite args obligatorios de Base → error
        let p = program_with_decls(
            vec![
                type_decl("Base",  vec![("x", Some(ty("Number")))], None, vec![], vec![]),
                type_decl("Child", vec![], Some("Base"), vec![], vec![]),
            ],
            new_("Child", vec![]),
        );
        check_err!(p, SemanticError::WrongArgCount { .. });
    }
}