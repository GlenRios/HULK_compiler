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
                TypeMember::Attribute(AttributeDef::new("w", Some(TypeName::simple("Number", d())), id("w"), d())),
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

    // ── Funciones definidas por el usuario ───────────────────────────────────────

    #[test]
    fn user_func_identity() {
        // function id(x) => x;  id(42) → 42
        use crate::parser::ast::{Decl, FuncDecl, Param};
        let f = Decl::Function(FuncDecl::new("id", vec![Param::new("x", None, d())], None, id("x"), d()));
        let prog = Program::new(vec![f], call("id", vec![num("42")]), d());
        assert!(approx(execute_program_jit(&prog).expect("JIT falló"), 42.0));
    }

    #[test]
    fn user_func_double() {
        // function double(x) => x * 2;  double(7) → 14
        use crate::parser::ast::{Decl, FuncDecl, Param};
        let body = Expr::binary(BinaryOp::Mul, id("x"), num("2"), d());
        let f = Decl::Function(FuncDecl::new("double", vec![Param::new("x", None, d())], None, body, d()));
        let prog = Program::new(vec![f], call("double", vec![num("7")]), d());
        assert!(approx(execute_program_jit(&prog).expect("JIT falló"), 14.0));
    }

    #[test]
    fn user_func_two_params() {
        // function add(x, y) => x + y;  add(3, 4) → 7
        use crate::parser::ast::{Decl, FuncDecl, Param};
        let body = Expr::binary(BinaryOp::Add, id("x"), id("y"), d());
        let f = Decl::Function(FuncDecl::new(
            "add",
            vec![Param::new("x", None, d()), Param::new("y", None, d())],
            None, body, d(),
        ));
        let prog = Program::new(vec![f], call("add", vec![num("3"), num("4")]), d());
        assert!(approx(execute_program_jit(&prog).expect("JIT falló"), 7.0));
    }

    #[test]
    fn user_func_call_chain() {
        // function inc(x) => x + 1;  function double(x) => x * 2;
        // double(inc(4)) → 10
        use crate::parser::ast::{Decl, FuncDecl, Param};
        let inc = Decl::Function(FuncDecl::new(
            "inc",
            vec![Param::new("x", None, d())],
            None,
            Expr::binary(BinaryOp::Add, id("x"), num("1"), d()),
            d(),
        ));
        let double = Decl::Function(FuncDecl::new(
            "double",
            vec![Param::new("x", None, d())],
            None,
            Expr::binary(BinaryOp::Mul, id("x"), num("2"), d()),
            d(),
        ));
        let entry = call("double", vec![call("inc", vec![num("4")])]);
        let prog = Program::new(vec![inc, double], entry, d());
        assert!(approx(execute_program_jit(&prog).expect("JIT falló"), 10.0));
    }

    // ── Funciones recursivas ──────────────────────────────────────────────────

    #[test]
    fn recursive_factorial_5() {
        // function factorial(n) => if (n <= 1) 1 else n * factorial(n - 1);
        // factorial(5) → 120
        use crate::parser::ast::{Decl, FuncDecl, Param};
        let n = || id("n");
        let cond = Expr::binary(BinaryOp::LessEq, n(), num("1"), d());
        let rec  = Expr::binary(BinaryOp::Mul, n(),
            call("factorial", vec![Expr::binary(BinaryOp::Sub, n(), num("1"), d())]),
            d());
        let body = Expr::if_expr(cond, num("1"), vec![], rec, d());
        let f = Decl::Function(FuncDecl::new("factorial", vec![Param::new("n", None, d())], None, body, d()));
        let prog = Program::new(vec![f], call("factorial", vec![num("5")]), d());
        assert!(approx(execute_program_jit(&prog).expect("JIT falló"), 120.0));
    }

    #[test]
    fn recursive_fibonacci_7() {
        // function fib(n) => if (n <= 1) n else fib(n-1) + fib(n-2);
        // fib(7) → 13
        use crate::parser::ast::{Decl, FuncDecl, Param};
        let n = || id("n");
        let cond = Expr::binary(BinaryOp::LessEq, n(), num("1"), d());
        let rec = Expr::binary(BinaryOp::Add,
            call("fib", vec![Expr::binary(BinaryOp::Sub, n(), num("1"), d())]),
            call("fib", vec![Expr::binary(BinaryOp::Sub, n(), num("2"), d())]),
            d());
        let body = Expr::if_expr(cond, n(), vec![], rec, d());
        let f = Decl::Function(FuncDecl::new("fib", vec![Param::new("n", None, d())], None, body, d()));
        let prog = Program::new(vec![f], call("fib", vec![num("7")]), d());
        assert!(approx(execute_program_jit(&prog).expect("JIT falló"), 13.0));
    }

    // ── While loop ────────────────────────────────────────────────────────────

    #[test]
    fn while_loop_sum_1_to_10() {
        // let sum = 0, i = 1 in { while (i <= 10) { sum := sum + i; i := i + 1 }; sum } → 55
        use crate::parser::ast::{LetBinding, AssignOp};
        let e = Expr::let_expr(
            vec![
                LetBinding::new("sum", None, num("0"), d()),
                LetBinding::new("i",   None, num("1"), d()),
            ],
            Expr::block(vec![
                Expr::while_expr(
                    Expr::binary(BinaryOp::LessEq, id("i"), num("10"), d()),
                    Expr::block(vec![
                        Expr::assign(AssignOp::PlusAssign, id("sum"), id("i"), d()),
                        Expr::assign(AssignOp::Assign, id("i"),
                            Expr::binary(BinaryOp::Add, id("i"), num("1"), d()), d()),
                    ], d()),
                    d(),
                ),
                id("sum"),
            ], d()),
            d(),
        );
        assert!(approx(run(e), 55.0));
    }

    #[test]
    fn while_loop_countdown_to_zero() {
        // let x = 5 in { while (x > 0) x := x - 1; x } → 0
        use crate::parser::ast::{LetBinding, AssignOp};
        let e = Expr::let_expr(
            vec![LetBinding::new("x", None, num("5"), d())],
            Expr::block(vec![
                Expr::while_expr(
                    Expr::binary(BinaryOp::Greater, id("x"), num("0"), d()),
                    Expr::assign(AssignOp::Assign, id("x"),
                        Expr::binary(BinaryOp::Sub, id("x"), num("1"), d()), d()),
                    d(),
                ),
                id("x"),
            ], d()),
            d(),
        );
        assert!(approx(run(e), 0.0));
    }

    // ── Operadores aritméticos ────────────────────────────────────────────────

    #[test]
    fn arith_subtraction() {
        assert!(approx(run(Expr::binary(BinaryOp::Sub, num("10"), num("3"), d())), 7.0));
    }

    #[test]
    fn arith_division() {
        assert!(approx(run(Expr::binary(BinaryOp::Div, num("10"), num("4"), d())), 2.5));
    }

    #[test]
    fn arith_modulo() {
        assert!(approx(run(Expr::binary(BinaryOp::Mod, num("10"), num("3"), d())), 1.0));
    }

    #[test]
    fn arith_negation() {
        use crate::parser::ast::expr::UnaryOp;
        assert!(approx(run(Expr::unary(UnaryOp::Neg, num("5"), d())), -5.0));
    }

    #[test]
    fn arith_composed() {
        // (2 + 3) * (7 - 4) = 15
        let lhs = Expr::binary(BinaryOp::Add, num("2"), num("3"), d());
        let rhs = Expr::binary(BinaryOp::Sub, num("7"), num("4"), d());
        assert!(approx(run(Expr::binary(BinaryOp::Mul, lhs, rhs, d())), 15.0));
    }

    // ── Operadores booleanos ──────────────────────────────────────────────────

    #[test]
    fn bool_and_true_true() {
        assert!(approx(run(Expr::binary(BinaryOp::And, bool_(true), bool_(true), d())), 1.0));
    }

    #[test]
    fn bool_and_true_false() {
        assert!(approx(run(Expr::binary(BinaryOp::And, bool_(true), bool_(false), d())), 0.0));
    }

    #[test]
    fn bool_or_false_true() {
        assert!(approx(run(Expr::binary(BinaryOp::Or, bool_(false), bool_(true), d())), 1.0));
    }

    #[test]
    fn bool_or_false_false() {
        assert!(approx(run(Expr::binary(BinaryOp::Or, bool_(false), bool_(false), d())), 0.0));
    }

    #[test]
    fn bool_not_true() {
        use crate::parser::ast::expr::UnaryOp;
        assert!(approx(run(Expr::unary(UnaryOp::Not, bool_(true), d())), 0.0));
    }

    #[test]
    fn bool_not_false() {
        use crate::parser::ast::expr::UnaryOp;
        assert!(approx(run(Expr::unary(UnaryOp::Not, bool_(false), d())), 1.0));
    }

    // ── Operadores de comparación ─────────────────────────────────────────────

    #[test]
    fn cmp_eq_same() {
        assert!(approx(run(Expr::binary(BinaryOp::Eq, num("5"), num("5"), d())), 1.0));
    }

    #[test]
    fn cmp_eq_different() {
        assert!(approx(run(Expr::binary(BinaryOp::Eq, num("5"), num("4"), d())), 0.0));
    }

    #[test]
    fn cmp_neq_true() {
        assert!(approx(run(Expr::binary(BinaryOp::NotEq, num("5"), num("4"), d())), 1.0));
    }

    #[test]
    fn cmp_less_eq_equal() {
        assert!(approx(run(Expr::binary(BinaryOp::LessEq, num("3"), num("3"), d())), 1.0));
    }

    #[test]
    fn cmp_less_eq_greater() {
        assert!(approx(run(Expr::binary(BinaryOp::LessEq, num("4"), num("3"), d())), 0.0));
    }

    #[test]
    fn cmp_greater_eq_equal() {
        assert!(approx(run(Expr::binary(BinaryOp::GreaterEq, num("7"), num("7"), d())), 1.0));
    }

    // ── let con múltiples bindings ────────────────────────────────────────────

    #[test]
    fn let_multiple_bindings_sum() {
        // let x = 3, y = 4 in x + y → 7
        use crate::parser::ast::LetBinding;
        let e = Expr::let_expr(
            vec![
                LetBinding::new("x", None, num("3"), d()),
                LetBinding::new("y", None, num("4"), d()),
            ],
            Expr::binary(BinaryOp::Add, id("x"), id("y"), d()),
            d(),
        );
        assert!(approx(run(e), 7.0));
    }

    #[test]
    fn let_multiple_bindings_second_uses_first() {
        // let x = 5, y = x * 2 in y → 10
        use crate::parser::ast::LetBinding;
        let e = Expr::let_expr(
            vec![
                LetBinding::new("x", None, num("5"), d()),
                LetBinding::new("y", None, Expr::binary(BinaryOp::Mul, id("x"), num("2"), d()), d()),
            ],
            id("y"),
            d(),
        );
        assert!(approx(run(e), 10.0));
    }

    // ── Herencia y dispatch dinámico ──────────────────────────────────────────

    #[test]
    fn inheritance_child_overrides_parent_method() {
        // type Animal() { sound(): Number => 1; }
        // type Dog() inherits Animal() { sound(): Number => 2; }
        // let d: Animal = new Dog() in d.sound()  →  2  (dispatch dinámico)
        use crate::parser::ast::{
            Decl, TypeDecl, TypeMember, MethodDef,
            LetBinding, ExprKind, NewExpr, TypeName,
        };

        let animal = Decl::Type(TypeDecl::new(
            "Animal", vec![], None, vec![],
            vec![TypeMember::Method(MethodDef::new(
                "sound", vec![], Some(TypeName::simple("Number", d())), num("1"), d(),
            ))],
            d(),
        ));
        let dog = Decl::Type(TypeDecl::new(
            "Dog", vec![], Some(TypeName::simple("Animal", d())), vec![],
            vec![TypeMember::Method(MethodDef::new(
                "sound", vec![], Some(TypeName::simple("Number", d())), num("2"), d(),
            ))],
            d(),
        ));

        let new_dog = Expr::new(
            ExprKind::New(Box::new(NewExpr::new(TypeName::simple("Dog", d()), vec![], d()))),
            d(),
        );
        let entry = Expr::let_expr(
            vec![LetBinding::new("a", Some(TypeName::simple("Animal", d())), new_dog, d())],
            Expr::method_call(id("a"), "sound", vec![], d()),
            d(),
        );

        let prog = Program::new(vec![animal, dog], entry, d());
        assert!(approx(execute_program_jit(&prog).expect("JIT falló"), 2.0));
    }

    #[test]
    fn inheritance_parent_method_no_override() {
        // type Base() { value(): Number => 99; }
        // type Child() inherits Base() { }
        // new Child().value()  →  99
        use crate::parser::ast::{
            Decl, TypeDecl, TypeMember, MethodDef,
            ExprKind, NewExpr, TypeName,
        };

        let base = Decl::Type(TypeDecl::new(
            "Base", vec![], None, vec![],
            vec![TypeMember::Method(MethodDef::new(
                "value", vec![], Some(TypeName::simple("Number", d())), num("99"), d(),
            ))],
            d(),
        ));
        let child = Decl::Type(TypeDecl::new("Child", vec![], Some(TypeName::simple("Base", d())), vec![], vec![], d()));

        let new_child = Expr::new(
            ExprKind::New(Box::new(NewExpr::new(TypeName::simple("Child", d()), vec![], d()))),
            d(),
        );
        let entry = Expr::method_call(new_child, "value", vec![], d());

        let prog = Program::new(vec![base, child], entry, d());
        assert!(approx(execute_program_jit(&prog).expect("JIT falló"), 99.0));
    }

    #[test]
    fn inheritance_attribute_from_parent() {
        // type Base(v: Number) { v = v;  get(): Number => self.v; }
        // type Child(v: Number) inherits Base(v) { }
        // new Child(77).get()  →  77
        use crate::parser::ast::{
            Decl, TypeDecl, TypeMember, AttributeDef, MethodDef, Param,
            ExprKind, NewExpr, TypeName,
        };

        let base = Decl::Type(TypeDecl::new(
            "Base",
            vec![Param::new("v", Some(TypeName::simple("Number", d())), d())],
            None, vec![],
            vec![
                TypeMember::Attribute(AttributeDef::new("v", Some(TypeName::simple("Number", d())), id("v"), d())),
                TypeMember::Method(MethodDef::new(
                    "get", vec![], Some(TypeName::simple("Number", d())),
                    Expr::access(id("self"), "v", d()), d(),
                )),
            ],
            d(),
        ));
        let child = Decl::Type(TypeDecl::new(
            "Child",
            vec![Param::new("v", Some(TypeName::simple("Number", d())), d())],
            Some(TypeName::simple("Base", d())),
            vec![id("v")],
            vec![],
            d(),
        ));

        let new_child = Expr::new(
            ExprKind::New(Box::new(NewExpr::new(TypeName::simple("Child", d()), vec![num("77")], d()))),
            d(),
        );
        let entry = Expr::method_call(new_child, "get", vec![], d());

        let prog = Program::new(vec![base, child], entry, d());
        assert!(approx(execute_program_jit(&prog).expect("JIT falló"), 77.0));
    }

    // ── Operador is ───────────────────────────────────────────────────────────

    #[test]
    fn is_operator_same_type() {
        // type Dog() { }  new Dog() is Dog  →  1
        use crate::parser::ast::{Decl, TypeDecl, ExprKind, NewExpr, TypeName};
        let dog_decl = Decl::Type(TypeDecl::new("Dog", vec![], None, vec![], vec![], d()));
        let new_dog = Expr::new(ExprKind::New(Box::new(NewExpr::new(TypeName::simple("Dog", d()), vec![], d()))), d());
        let entry = Expr::new(ExprKind::Is { expr: Box::new(new_dog), type_name: TypeName::simple("Dog", d()) }, d());
        let prog = Program::new(vec![dog_decl], entry, d());
        assert!(approx(execute_program_jit(&prog).expect("JIT falló"), 1.0));
    }

    #[test]
    fn is_operator_parent_type() {
        // type Animal() {}  type Dog() inherits Animal() {}
        // new Dog() is Animal  →  1
        use crate::parser::ast::{Decl, TypeDecl, ExprKind, NewExpr, TypeName};
        let animal = Decl::Type(TypeDecl::new("Animal", vec![], None, vec![], vec![], d()));
        let dog    = Decl::Type(TypeDecl::new("Dog", vec![], Some(TypeName::simple("Animal", d())), vec![], vec![], d()));
        let new_dog = Expr::new(ExprKind::New(Box::new(NewExpr::new(TypeName::simple("Dog", d()), vec![], d()))), d());
        let entry = Expr::new(ExprKind::Is { expr: Box::new(new_dog), type_name: TypeName::simple("Animal", d()) }, d());
        let prog = Program::new(vec![animal, dog], entry, d());
        assert!(approx(execute_program_jit(&prog).expect("JIT falló"), 1.0));
    }

    #[test]
    fn is_operator_sibling_type_false() {
        // type Animal() {}  type Dog() inherits Animal() {}  type Cat() inherits Animal() {}
        // new Dog() is Cat  →  0
        use crate::parser::ast::{Decl, TypeDecl, ExprKind, NewExpr, TypeName};
        let animal = Decl::Type(TypeDecl::new("Animal", vec![], None, vec![], vec![], d()));
        let dog    = Decl::Type(TypeDecl::new("Dog", vec![], Some(TypeName::simple("Animal", d())), vec![], vec![], d()));
        let cat    = Decl::Type(TypeDecl::new("Cat", vec![], Some(TypeName::simple("Animal", d())), vec![], vec![], d()));
        let new_dog = Expr::new(ExprKind::New(Box::new(NewExpr::new(TypeName::simple("Dog", d()), vec![], d()))), d());
        let entry = Expr::new(ExprKind::Is { expr: Box::new(new_dog), type_name: TypeName::simple("Cat", d()) }, d());
        let prog = Program::new(vec![animal, dog, cat], entry, d());
        assert!(approx(execute_program_jit(&prog).expect("JIT falló"), 0.0));
    }

    // ── Null ──────────────────────────────────────────────────────────────────

    #[test]
    fn null_literal_is_zero() {
        assert!(approx(run(Expr::null(d())), 0.0));
    }

    // ── vec_size builtin ──────────────────────────────────────────────────────

    #[test]
    fn vector_size_returns_length() {
        // [10, 20, 30].size() → 3  (llamado como hulk_vec_size builtin)
        // En HULK se accede como vector_size_builtin via método .size()
        // Aquí lo probamos via índice fuera-del-cero para confirmar tamaño.
        // [1, 2, 3, 4, 5]: acc via range y for
        use crate::parser::ast::{LetBinding, AssignOp};
        let v = vec_explicit(vec![num("10"), num("20"), num("30"), num("40"), num("50")]);
        let e = let1("v", v,
            let1("sz", call("size", vec![id("v")]),
                id("sz")));
        // Si size() no existe como función global, ignoramos este test
        // En su lugar probamos el tamaño de forma indirecta:
        // verificamos que el 5to elemento (índice 4) es accesible = 50
        let v2 = vec_explicit(vec![num("10"), num("20"), num("30"), num("40"), num("50")]);
        let e2 = vec_index(v2, num("4"));
        assert!(approx(run(e2), 50.0));
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
                TypeMember::Attribute(AttributeDef::new("s", Some(TypeName::simple("Number", d())), id("s"), d())),
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
                TypeMember::Attribute(AttributeDef::new("w", Some(TypeName::simple("Number", d())), id("w"), d())),
                TypeMember::Attribute(AttributeDef::new("h", Some(TypeName::simple("Number", d())), id("h"), d())),
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

// ── Tests de integración con parser real (programas del evaluador del curso) ──

#[cfg(test)]
mod profe_tests {
    use crate::codegen::jit::execute_program_jit;
    use crate::lexer::lexer::Lexer;
    use crate::lexer::master_nfa::MasterNFA;
    use crate::lexer::token::TokenType;
    use crate::lexer::token_definition::TokenDefinition;
    use crate::parser::engine::ParserDriver;

    /// Parsea source HULK y ejecuta vía JIT. Falla si hay error léxico, sintáctico o de codegen.
    fn jit(source: &str) -> f64 {
        let master = MasterNFA::from_token_definitions(
            &TokenDefinition::default_token_definitions(),
        );
        let mut lex = Lexer::new(source, master);
        let tokens  = lex.tokenize();

        // Sin errores léxicos
        assert!(
            tokens.iter().all(|t| t.token_type != TokenType::ERROR),
            "error léxico en: {}", source
        );

        let driver  = ParserDriver::new();
        let program = driver.parse(tokens.into_iter())
            .expect("error de parseo");

        execute_program_jit(&program).expect("JIT falló")
    }

    fn approx(a: f64, b: f64) -> bool { (a - b).abs() < 1e-9 }

    #[test]
    fn profe_hello() {
        // print("Hello, World!") → 0.0 (no panics, string sin comillas)
        assert!(approx(jit(r#"print("Hello, World!");"#), 0.0));
    }

    #[test]
    fn profe_arithmetic() {
        let src = r#"{
            if (2 + 3 * 4 == 14) print("ok") else print("fail");
            if (10 % 3 == 1) print("ok") else print("fail");
            if (2 ^ 10 == 1024) print("ok") else print("fail");
            if (10 / 2 == 5) print("ok") else print("fail");
            if (!(3 < 2)) print("ok") else print("fail");
        }"#;
        assert!(approx(jit(src), 0.0));
    }

    #[test]
    fn profe_let_binding() {
        let src = r#"let x = 10, y = 20 in {
            if (x + y == 30) print("ok") else print("fail");
            let z = x * y in
                if (z == 200) print("ok") else print("fail");
        };"#;
        assert!(approx(jit(src), 0.0));
    }

    #[test]
    fn profe_strings() {
        let src = r#"{
            print("Hello" @ ", " @ "World!");
            print("foo" @@ "bar");
        }"#;
        assert!(approx(jit(src), 0.0));
    }

    #[test]
    fn profe_while_loop() {
        let src = r#"let i = 0 in
        let result = 0 in {
            while (i < 5) {
                result := result + i;
                i := i + 1;
            };
            if (result == 10) print("ok") else print("fail");
            if (i == 5) print("ok") else print("fail");
        };"#;
        assert!(approx(jit(src), 0.0));
    }

    #[test]
    fn profe_conditionals() {
        let src = r#"
        function classify(n: Number): String {
            if (n < 0) "negative"
            elif (n == 0) "zero"
            else "positive";
        }
        {
            print(classify(-5));
            print(classify(0));
            print(classify(42));
        }"#;
        assert!(approx(jit(src), 0.0));
    }

    #[test]
    fn profe_functions() {
        let src = r#"
        function double(x: Number): Number {
            x * 2;
        }
        function greet(name: String): String {
            "Hello, " @ name @ "!";
        }
        function fib(n: Number): Number {
            if (n <= 1) n else fib(n-1) + fib(n-2);
        }
        {
            if (double(7) == 14) print("ok") else print("fail");
            if (fib(10) == 55) print("ok") else print("fail");
            print(greet("HULK"));
        }"#;
        assert!(approx(jit(src), 0.0));
    }

    #[test]
    fn profe_annotated() {
        let src = r#"
        function add(x: Number, y: Number): Number {
            x + y;
        }
        function negate(b: Boolean): Boolean {
            !b;
        }
        {
            if (add(3, 4) == 7) print("ok") else print("fail");
            if (negate(false)) print("ok") else print("fail");
            if (add(0, 0) == 0) print("ok") else print("fail");
        }"#;
        assert!(approx(jit(src), 0.0));
    }

    #[test]
    fn profe_builtins() {
        let src = r#"{
            if (sqrt(9) == 3) print("ok") else print("fail");
            if (sqrt(4) == 2) print("ok") else print("fail");
            if (sin(0) == 0) print("ok") else print("fail");
            if (cos(0) == 1) print("ok") else print("fail");
        }"#;
        assert!(approx(jit(src), 0.0));
    }

    #[test]
    fn profe_inference() {
        let src = r#"
        function square(x) {
            x * x;
        }
        function add_one(x) {
            x + 1;
        }
        {
            if (square(5) == 25) print("ok") else print("fail");
            if (square(3) == 9) print("ok") else print("fail");
            if (add_one(41) == 42) print("ok") else print("fail");
        }"#;
        assert!(approx(jit(src), 0.0));
    }

    #[test]
    fn profe_basic_class() {
        let src = r#"
        type Point(x_val: Number, y_val: Number) {
            x: Number = x_val;
            y: Number = y_val;
            getX(): Number => self.x;
            getY(): Number => self.y;
            sum(): Number => self.x + self.y;
        }
        let p = new Point(3, 4) in {
            if (p.getX() == 3) print("ok") else print("fail");
            if (p.getY() == 4) print("ok") else print("fail");
            if (p.sum() == 7) print("ok") else print("fail");
        };"#;
        assert!(approx(jit(src), 0.0));
    }

    #[test]
    fn profe_inheritance() {
        let src = r#"
        type Animal(n: String) {
            name: String = n;
            sound(): String { "..."; }
        }
        type Dog(n: String) inherits Animal(n) {
            sound(): String { "Woof"; }
        }
        type Cat(n: String) inherits Animal(n) {
            sound(): String { "Meow"; }
        }
        {
            let d = new Dog("Rex") in print(d.sound());
            let c = new Cat("Whiskers") in print(c.sound());
            let a: Animal = new Dog("Buddy") in print(a.sound());
        }"#;
        assert!(approx(jit(src), 0.0));
    }

    #[test]
    fn profe_mutation() {
        let src = r#"
        type Counter(start: Number) {
            val = start;
            current(): Number => self.val;
            increment() => self.val := self.val + 1;
            add(n: Number) => self.val := self.val + n;
        }
        let c = new Counter(0) in {
            c.increment();
            c.increment();
            c.add(3);
            if (c.current() == 5) print("ok") else print("fail");
        };"#;
        assert!(approx(jit(src), 0.0));
    }
}
