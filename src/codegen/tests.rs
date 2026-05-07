// src/codegen/tests.rs
//
// Tests de integración del backend LLVM: construyen AST directamente,
// lo compilan con JIT y verifican el resultado numérico.

#[cfg(test)]
mod codegen_tests {
    use crate::parser::ast::{BinaryOp, ElifBranch, Expr, Program, Span};
    use crate::codegen::jit::execute_program_jit;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn d() -> Span { Span::dummy() }
    fn num(v: &str) -> Expr { Expr::number(v, d()) }
    fn bool_(v: bool) -> Expr { Expr::bool(v, d()) }
    fn pow(l: Expr, r: Expr) -> Expr { Expr::binary(BinaryOp::Power, l, r, d()) }

    fn run(entry: Expr) -> f64 {
        let prog = Program::new(vec![], entry, d());
        execute_program_jit(&prog).expect("JIT falló")
    }

    fn approx(a: f64, b: f64) -> bool { (a - b).abs() < 1e-9 }

    fn call(name: &str, args: Vec<Expr>) -> Expr {
        Expr::call(Expr::identifier(name, d()), args, d())
    }

    fn str_(v: &str) -> Expr { Expr::string(v, d()) }
    fn concat(l: Expr, r: Expr) -> Expr { Expr::binary(BinaryOp::Concat, l, r, d()) }
    fn dconcat(l: Expr, r: Expr) -> Expr { Expr::binary(BinaryOp::DoubleConcat, l, r, d()) }

    // ── Strings ───────────────────────────────────────────────────────────────

    #[test]
    fn print_string_literal() {
        // print("hola") devuelve Null → 0.0
        assert!(approx(run(call("print", vec![str_("hola")])), 0.0));
    }

    #[test]
    fn string_concat_via_print() {
        // print("a" @ "b") → 0.0 (no crashea)
        assert!(approx(run(call("print", vec![concat(str_("a"), str_("b"))])), 0.0));
    }

    #[test]
    fn string_concat_number() {
        // print("x=" @ 42) → 0.0 (número auto-convertido a string)
        assert!(approx(run(call("print", vec![concat(str_("x="), num("42"))])), 0.0));
    }

    #[test]
    fn string_concat_bool() {
        // print("val=" @ true) → 0.0 (bool auto-convertido)
        assert!(approx(run(call("print", vec![concat(str_("val="), bool_(true))])), 0.0));
    }

    #[test]
    fn double_concat_adds_space() {
        // print("hello" @@ "world") → 0.0 (no crashea)
        assert!(approx(run(call("print", vec![dconcat(str_("hello"), str_("world"))])), 0.0));
    }

    #[test]
    fn let_string_variable() {
        // let s = "hola" in print(s)  →  0.0
        let e = Expr::let_expr(
            vec![crate::parser::ast::LetBinding::new("s", None, str_("hola"), d())],
            call("print", vec![Expr::identifier("s", d())]),
            d(),
        );
        assert!(approx(run(e), 0.0));
    }

    #[test]
    fn string_reassign() {
        // let s = "a" in { s := "b"; print(s) }  →  0.0
        let e = Expr::let_expr(
            vec![crate::parser::ast::LetBinding::new("s", None, str_("a"), d())],
            Expr::block(vec![
                Expr::assign(
                    crate::parser::ast::AssignOp::Assign,
                    Expr::identifier("s", d()),
                    str_("b"),
                    d(),
                ),
                call("print", vec![Expr::identifier("s", d())]),
            ], d()),
            d(),
        );
        assert!(approx(run(e), 0.0));
    }

    // ── Builtins matemáticos ──────────────────────────────────────────────────

    #[test]
    fn builtin_sqrt_9() {
        assert!(approx(run(call("sqrt", vec![num("9")])), 3.0));
    }

    #[test]
    fn builtin_sqrt_0() {
        assert!(approx(run(call("sqrt", vec![num("0")])), 0.0));
    }

    #[test]
    fn builtin_sin_zero() {
        assert!(approx(run(call("sin", vec![num("0")])), 0.0));
    }

    #[test]
    fn builtin_cos_zero() {
        assert!(approx(run(call("cos", vec![num("0")])), 1.0));
    }

    #[test]
    fn builtin_exp_zero() {
        assert!(approx(run(call("exp", vec![num("0")])), 1.0));
    }

    #[test]
    fn builtin_log_base10_of_100() {
        // log(10, 100) = 2.0
        assert!(approx(run(call("log", vec![num("10"), num("100")])), 2.0));
    }

    #[test]
    fn builtin_log_base2_of_8() {
        // log(2, 8) = 3.0
        assert!(approx(run(call("log", vec![num("2"), num("8")])), 3.0));
    }

    #[test]
    fn builtin_rand_in_range() {
        // rand() debe estar en [0.0, 1.0]
        let v = run(call("rand", vec![]));
        assert!(v >= 0.0 && v <= 1.0, "rand() = {v} fuera de [0,1]");
    }

    #[test]
    fn builtin_print_returns_null_as_zero() {
        // print devuelve Null → require_number lo convierte a 0.0
        let e = call("print", vec![num("42")]);
        assert!(approx(run(e), 0.0));
    }

    #[test]
    fn builtin_sqrt_composed_with_arithmetic() {
        // sqrt(9) + 1 = 4.0
        let e = Expr::binary(
            BinaryOp::Add,
            call("sqrt", vec![num("9")]),
            num("1"),
            d(),
        );
        assert!(approx(run(e), 4.0));
    }

    // ── Power ─────────────────────────────────────────────────────────────────

    #[test]
    fn power_2_to_10() {
        assert!(approx(run(pow(num("2"), num("10"))), 1024.0));
    }

    #[test]
    fn power_sqrt_via_half_exponent() {
        // 4^0.5 = 2.0
        assert!(approx(run(pow(num("4"), num("0.5"))), 2.0));
    }

    #[test]
    fn power_zero_exponent() {
        // x^0 = 1 para cualquier x != 0
        assert!(approx(run(pow(num("99"), num("0"))), 1.0));
    }

    #[test]
    fn power_one_exponent() {
        assert!(approx(run(pow(num("7"), num("1"))), 7.0));
    }

    #[test]
    fn power_chained() {
        // (2^3)^2 = 64
        assert!(approx(run(pow(pow(num("2"), num("3")), num("2"))), 64.0));
    }

    // ── Bool en variables (VarSlot) ───────────────────────────────────────────

    #[test]
    fn let_bool_true_used_as_condition() {
        // let x = true in if (x) 1 else 2  →  1
        let x = Expr::identifier("x", d());
        let e = Expr::let_expr(
            vec![crate::parser::ast::LetBinding::new("x", None, bool_(true), d())],
            Expr::if_expr(x, num("1"), vec![], num("2"), d()),
            d(),
        );
        assert!(approx(run(e), 1.0));
    }

    #[test]
    fn let_bool_false_used_as_condition() {
        // let x = false in if (x) 1 else 2  →  2
        let x = Expr::identifier("x", d());
        let e = Expr::let_expr(
            vec![crate::parser::ast::LetBinding::new("x", None, bool_(false), d())],
            Expr::if_expr(x, num("1"), vec![], num("2"), d()),
            d(),
        );
        assert!(approx(run(e), 2.0));
    }

    #[test]
    fn comparison_result_stored_in_variable() {
        // let cond = (3 > 2) in if (cond) 10 else 20  →  10
        let cond_expr = Expr::binary(BinaryOp::Greater, num("3"), num("2"), d());
        let cond_var  = Expr::identifier("cond", d());
        let e = Expr::let_expr(
            vec![crate::parser::ast::LetBinding::new("cond", None, cond_expr, d())],
            Expr::if_expr(cond_var, num("10"), vec![], num("20"), d()),
            d(),
        );
        assert!(approx(run(e), 10.0));
    }

    #[test]
    fn if_returns_bool_branch() {
        // if (true) true else false  → entry lo convierte a 1.0
        let e = Expr::if_expr(bool_(true), bool_(true), vec![], bool_(false), d());
        assert!(approx(run(e), 1.0));
    }

    #[test]
    fn if_returns_bool_false_branch() {
        // if (false) true else false  → 0.0
        let e = Expr::if_expr(bool_(false), bool_(true), vec![], bool_(false), d());
        assert!(approx(run(e), 0.0));
    }

    // ── If / else (no regresión) ──────────────────────────────────────────────

    #[test]
    fn if_else_true_branch() {
        let e = Expr::if_expr(bool_(true), num("1"), vec![], num("2"), d());
        assert!(approx(run(e), 1.0));
    }

    #[test]
    fn if_else_false_branch() {
        let e = Expr::if_expr(bool_(false), num("1"), vec![], num("2"), d());
        assert!(approx(run(e), 2.0));
    }

    // ── Elif ──────────────────────────────────────────────────────────────────

    #[test]
    fn elif_first_branch_taken() {
        // if (true) 1 elif (true) 2 else 3  →  1
        let e = Expr::if_expr(
            bool_(true), num("1"),
            vec![ElifBranch::new(bool_(true), num("2"), d())],
            num("3"), d(),
        );
        assert!(approx(run(e), 1.0));
    }

    #[test]
    fn elif_second_branch_taken() {
        // if (false) 1 elif (true) 2 else 3  →  2
        let e = Expr::if_expr(
            bool_(false), num("1"),
            vec![ElifBranch::new(bool_(true), num("2"), d())],
            num("3"), d(),
        );
        assert!(approx(run(e), 2.0));
    }

    #[test]
    fn elif_else_branch_taken() {
        // if (false) 1 elif (false) 2 else 3  →  3
        let e = Expr::if_expr(
            bool_(false), num("1"),
            vec![ElifBranch::new(bool_(false), num("2"), d())],
            num("3"), d(),
        );
        assert!(approx(run(e), 3.0));
    }

    #[test]
    fn elif_multiple_branches_middle_taken() {
        // if (false) 1 elif (false) 2 elif (true) 3 else 4  →  3
        let e = Expr::if_expr(
            bool_(false), num("1"),
            vec![
                ElifBranch::new(bool_(false), num("2"), d()),
                ElifBranch::new(bool_(true),  num("3"), d()),
            ],
            num("4"), d(),
        );
        assert!(approx(run(e), 3.0));
    }

    #[test]
    fn elif_multiple_branches_else_taken() {
        // if (false) 1 elif (false) 2 elif (false) 3 else 4  →  4
        let e = Expr::if_expr(
            bool_(false), num("1"),
            vec![
                ElifBranch::new(bool_(false), num("2"), d()),
                ElifBranch::new(bool_(false), num("3"), d()),
            ],
            num("4"), d(),
        );
        assert!(approx(run(e), 4.0));
    }

    #[test]
    fn elif_first_true_wins_over_later_true() {
        // if (true) 10 elif (true) 20 else 30  →  10  (primer true gana)
        let e = Expr::if_expr(
            bool_(true), num("10"),
            vec![ElifBranch::new(bool_(true), num("20"), d())],
            num("30"), d(),
        );
        assert!(approx(run(e), 10.0));
    }

    // ── Vectores, Range y For ─────────────────────────────────────────────────

    fn id(name: &str) -> Expr { Expr::identifier(name, d()) }

    fn vec_explicit(elems: Vec<Expr>) -> Expr {
        use crate::parser::ast::{ExprKind, VectorExpr};
        Expr::new(ExprKind::Vector(Box::new(VectorExpr::explicit(elems, d()))), d())
    }

    fn vec_gen(body: Expr, var: &str, iterable: Expr) -> Expr {
        use crate::parser::ast::{ExprKind, VectorExpr};
        Expr::new(ExprKind::Vector(Box::new(VectorExpr::generator(body, var, iterable, d()))), d())
    }

    fn vec_index(coll: Expr, idx: Expr) -> Expr { Expr::index(coll, idx, d()) }

    fn range_call(start: Expr, end: Expr) -> Expr { call("range", vec![start, end]) }

    fn for_loop(var: &str, iterable: Expr, body: Expr) -> Expr {
        Expr::for_expr(var, iterable, body, d())
    }

    fn let1(var: &str, val: Expr, body: Expr) -> Expr {
        use crate::parser::ast::LetBinding;
        Expr::let_expr(vec![LetBinding::new(var, None, val, d())], body, d())
    }

    #[test]
    fn vector_explicit_index() {
        // [10, 20, 30][1]  →  20
        let e = vec_index(
            vec_explicit(vec![num("10"), num("20"), num("30")]),
            num("1"));
        assert!(approx(run(e), 20.0));
    }

    #[test]
    fn vector_explicit_first_element() {
        // [42, 0, 0][0]  →  42
        let e = vec_index(
            vec_explicit(vec![num("42"), num("0"), num("0")]),
            num("0"));
        assert!(approx(run(e), 42.0));
    }

    #[test]
    fn vector_explicit_last_element() {
        // [1, 2, 99][2]  →  99
        let e = vec_index(
            vec_explicit(vec![num("1"), num("2"), num("99")]),
            num("2"));
        assert!(approx(run(e), 99.0));
    }

    #[test]
    fn for_range_runs_body() {
        // for (x in range(0, 5)) 0  →  0.0 (no crash)
        let e = for_loop("x", range_call(num("0"), num("5")), num("0"));
        assert!(approx(run(e), 0.0));
    }

    #[test]
    fn for_range_accumulates_via_let() {
        // let total = 0 in { for (x in range(1,4)) total := total + x; total }  →  6
        use crate::parser::ast::AssignOp;
        let e = let1("total", num("0"),
            Expr::block(vec![
                for_loop("x", range_call(num("1"), num("4")),
                    Expr::assign(AssignOp::PlusAssign, id("total"), id("x"), d())),
                id("total"),
            ], d()));
        assert!(approx(run(e), 6.0));
    }

    #[test]
    fn vector_generator_first_element() {
        // [x^2 | x in range(1, 4)][0]  →  1  (1^2)
        let e = vec_index(
            vec_gen(pow(id("x"), num("2")), "x", range_call(num("1"), num("4"))),
            num("0"));
        assert!(approx(run(e), 1.0));
    }

    #[test]
    fn vector_generator_second_element() {
        // [x^2 | x in range(1, 4)][1]  →  4  (2^2)
        let e = vec_index(
            vec_gen(pow(id("x"), num("2")), "x", range_call(num("1"), num("4"))),
            num("1"));
        assert!(approx(run(e), 4.0));
    }

    #[test]
    fn vector_generator_third_element() {
        // [x^2 | x in range(1, 4)][2]  →  9  (3^2)
        let e = vec_index(
            vec_gen(pow(id("x"), num("2")), "x", range_call(num("1"), num("4"))),
            num("2"));
        assert!(approx(run(e), 9.0));
    }

    #[test]
    fn vector_generator_from_vector() {
        // let v = [10, 20, 30] in [x*2 | x in v][2]  →  60
        let e = let1("v",
            vec_explicit(vec![num("10"), num("20"), num("30")]),
            vec_index(
                vec_gen(
                    Expr::binary(BinaryOp::Mul, id("x"), num("2"), d()),
                    "x", id("v")),
                num("2")));
        assert!(approx(run(e), 60.0));
    }

    // ── Protocolos ────────────────────────────────────────────────────────────

    #[test]
    fn protocol_dispatch_single_conformer() {
        use crate::parser::ast::{
            Decl, ProtocolDecl, TypeDecl, TypeMember, AttributeDef, MethodDef,
            MethodSignature, Param, LetBinding, ExprKind, NewExpr, TypeName,
        };

        // protocol Measurable { measure(): Number; }
        let proto = Decl::Protocol(ProtocolDecl::new(
            "Measurable",
            None,
            vec![MethodSignature::new("measure", vec![], TypeName::simple("Number", d()), d())],
            d(),
        ));

        // type MBox(w: Number) { w = w; measure(): Number => self.w; }
        let typ = Decl::Type(TypeDecl::new(
            "MBox",
            vec![Param::new("w", Some(TypeName::simple("Number", d())), d())],
            None,
            vec![],
            vec![
                TypeMember::Attribute(AttributeDef::new("w", None, id("w"), d())),
                TypeMember::Method(MethodDef::new(
                    "measure",
                    vec![],
                    Some(TypeName::simple("Number", d())),
                    Expr::access(id("self"), "w", d()),
                    d(),
                )),
            ],
            d(),
        ));

        // let b: Measurable = new MBox(42) in b.measure()  →  42
        let new_mbox = Expr::new(
            ExprKind::New(Box::new(NewExpr::new(TypeName::simple("MBox", d()), vec![num("42")], d()))),
            d(),
        );
        let entry = Expr::let_expr(
            vec![LetBinding::new("b", Some(TypeName::simple("Measurable", d())), new_mbox, d())],
            Expr::method_call(id("b"), "measure", vec![], d()),
            d(),
        );

        let prog = Program::new(vec![proto, typ], entry, d());
        assert!(approx(execute_program_jit(&prog).expect("JIT falló"), 42.0));
    }

    #[test]
    fn protocol_dispatch_multiple_conformers() {
        use crate::parser::ast::{
            Decl, ProtocolDecl, TypeDecl, TypeMember, AttributeDef, MethodDef,
            MethodSignature, Param, LetBinding, ExprKind, NewExpr, TypeName,
        };

        // protocol Shape { area(): Number; }
        let proto = Decl::Protocol(ProtocolDecl::new(
            "Shape",
            None,
            vec![MethodSignature::new("area", vec![], TypeName::simple("Number", d()), d())],
            d(),
        ));

        // type Square(s: Number) { s = s; area(): Number => self.s * self.s; }
        let square_decl = Decl::Type(TypeDecl::new(
            "Square",
            vec![Param::new("s", Some(TypeName::simple("Number", d())), d())],
            None,
            vec![],
            vec![
                TypeMember::Attribute(AttributeDef::new("s", None, id("s"), d())),
                TypeMember::Method(MethodDef::new(
                    "area",
                    vec![],
                    Some(TypeName::simple("Number", d())),
                    Expr::binary(BinaryOp::Mul,
                        Expr::access(id("self"), "s", d()),
                        Expr::access(id("self"), "s", d()),
                        d()),
                    d(),
                )),
            ],
            d(),
        ));

        // type Rect(w: Number, h: Number) { w = w; h = h; area(): Number => self.w * self.h; }
        let rect_decl = Decl::Type(TypeDecl::new(
            "Rect",
            vec![
                Param::new("w", Some(TypeName::simple("Number", d())), d()),
                Param::new("h", Some(TypeName::simple("Number", d())), d()),
            ],
            None,
            vec![],
            vec![
                TypeMember::Attribute(AttributeDef::new("w", None, id("w"), d())),
                TypeMember::Attribute(AttributeDef::new("h", None, id("h"), d())),
                TypeMember::Method(MethodDef::new(
                    "area",
                    vec![],
                    Some(TypeName::simple("Number", d())),
                    Expr::binary(BinaryOp::Mul,
                        Expr::access(id("self"), "w", d()),
                        Expr::access(id("self"), "h", d()),
                        d()),
                    d(),
                )),
            ],
            d(),
        ));

        // let shape: Shape = new Square(5) in shape.area()  →  25
        let new_square = Expr::new(
            ExprKind::New(Box::new(NewExpr::new(TypeName::simple("Square", d()), vec![num("5")], d()))),
            d(),
        );
        let entry = Expr::let_expr(
            vec![LetBinding::new("shape", Some(TypeName::simple("Shape", d())), new_square, d())],
            Expr::method_call(id("shape"), "area", vec![], d()),
            d(),
        );

        // Ambos tipos registrados → switch con 2 arms; en runtime Square(5) → 25
        let prog = Program::new(vec![proto, square_decl, rect_decl], entry, d());
        assert!(approx(execute_program_jit(&prog).expect("JIT falló"), 25.0));
    }
}
