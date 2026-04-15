// src/semantic/tests.rs
//
// Suite completa de tests del analizador semántico.
// Cada test construye un AST directamente (sin pasar por el parser)
// y verifica que el TypeChecker produce los errores esperados — o ninguno.
//
// Convención:
//   check_ok!(program)         → el programa debe pasar sin errores
//   check_err!(program, Kind)  → debe producir al menos un error de ese Kind

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
 
fn type_decl(
    name: &str,
    args: Vec<(&str, Option<TypeName>)>,
    parent: Option<&str>,
    members: Vec<TypeMember>,
) -> Decl {
    let ps = args.into_iter().map(|(n, t)| Param::new(n, t, d())).collect();
    let par = parent.map(|p| ty(p));
    Decl::Type(TypeDecl::new(name, ps, par, vec![], members, d()))
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
            Ok(()) => vec![],
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
            Ok(()) => vec![],
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
    ($prog:expr, $check:expr) => {{
        let errors = match analyze(&$prog) {
            Ok(()) => vec![],
            Err(errors) => errors,
        };
        assert!(
            !errors.iter().any(|e| $check(e)),
            "No se esperaba ese error pero apareció:\n{}",
            errors.iter().map(|e| format!("  • {}", e)).collect::<Vec<_>>().join("\n")
        );
    }};
}
 
macro_rules! error_count {
    ($prog:expr) => {{
        match analyze(&$prog) {
            Ok(()) => 0,
            Err(errors) => errors.len(),
        }
    }};
}

#[test]
fn literal_number_ok() { check_ok!(simple_program(num("42"))); }
#[test]
fn literal_string_ok() { check_ok!(simple_program(str_("hola"))); }
#[test]
fn literal_bool_ok() { check_ok!(simple_program(bool_(true))); }
#[test]
fn literal_null_ok() { check_ok!(simple_program(null())); }

#[test]
fn identifier_undefined_error() {
    check_err!(simple_program(id("x")), SemanticError::UndefinedVariable { .. });
}
#[test]
fn identifier_builtin_pi_ok() { check_ok!(simple_program(id("PI"))); }
#[test]
fn identifier_builtin_e_ok() { check_ok!(simple_program(id("E"))); }
#[test]
fn identifier_true_false_ok() {
    check_ok!(simple_program(id("true")));
    check_ok!(simple_program(id("false")));
}

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
    check_err!(simple_program(add(str_("hola"), num("1"))), SemanticError::InvalidBinaryTypes { .. });
}
#[test]
fn binary_arithmetic_bool_error() {
    check_err!(simple_program(sub(bool_(true), num("1"))), SemanticError::InvalidBinaryTypes { .. });
}
#[test]
fn binary_logical_ok() {
    check_ok!(simple_program(and(bool_(true), bool_(false))));
    check_ok!(simple_program(or_(bool_(true), bool_(true))));
}
#[test]
fn binary_logical_wrong_types() {
    check_err!(simple_program(and(num("1"), bool_(true))), SemanticError::InvalidBinaryTypes { .. });
}
#[test]
fn binary_comparison_numbers_ok() {
    check_ok!(simple_program(lt_(num("1"), num("2"))));
    check_ok!(simple_program(Expr::binary(BinaryOp::Greater, num("5"), num("3"), d())));
    check_ok!(simple_program(Expr::binary(BinaryOp::LessEq, num("1"), num("1"), d())));
    check_ok!(simple_program(Expr::binary(BinaryOp::GreaterEq, num("2"), num("1"), d())));
}
#[test]
fn binary_comparison_string_error() {
    check_err!(simple_program(lt_(str_("a"), str_("b"))), SemanticError::InvalidBinaryTypes { .. });
}
#[test]
fn binary_equality_same_types_ok() {
    check_ok!(simple_program(eq_(num("1"), num("1"))));
    check_ok!(simple_program(eq_(bool_(true), bool_(false))));
}
#[test]
fn binary_equality_incompatible_types_error() {
    check_err!(simple_program(eq_(num("1"), str_("x"))), SemanticError::InvalidBinaryTypes { .. });
}
#[test]
fn binary_concat_ok() {
    check_ok!(simple_program(cat(str_("hola"), num("42"))));
    check_ok!(simple_program(cat(str_("x"), str_("y"))));
    check_ok!(simple_program(Expr::binary(BinaryOp::DoubleConcat, str_("a"), str_("b"), d())));
}

#[test]
fn unary_neg_number_ok() { check_ok!(simple_program(neg(num("5")))); }
#[test]
fn unary_neg_wrong_type() {
    check_err!(simple_program(neg(bool_(true))), SemanticError::InvalidOperandType { .. });
}
#[test]
fn unary_not_bool_ok() { check_ok!(simple_program(not(bool_(true)))); }
#[test]
fn unary_not_wrong_type() {
    check_err!(simple_program(not(num("1"))), SemanticError::InvalidOperandType { .. });
}
#[test]
fn postfix_increment_number_ok() {
    let prog = program_with_decls(
        vec![],
        let_(vec![("x", None, num("0"))],
            Expr::Postfix(Box::new(crate::parser::ast::PostfixExpr::new(PostfixOp::Increment, id("x"), d()))))
    );
    check_ok!(prog);
}
#[test]
fn postfix_on_bool_error() {
    check_err!(
        simple_program(Expr::Postfix(Box::new(crate::parser::ast::PostfixExpr::new(PostfixOp::Increment, bool_(true), d())))),
        SemanticError::InvalidOperandType { .. }
    );
}

#[test]
fn block_type_is_last_expr() { check_ok!(simple_program(block(vec![num("1"), str_("hola"), bool_(true)]))); }
#[test]
fn block_single_expr_ok() { check_ok!(simple_program(block(vec![num("42")] ))); }
#[test]
fn block_propagates_inner_errors() {
    check_err!(simple_program(block(vec![num("1"), id("undefined_var")])), SemanticError::UndefinedVariable { .. });
}

#[test]
fn let_simple_ok() { check_ok!(simple_program(let_(vec![("x", None, num("42"))], id("x")))); }
#[test]
fn let_with_type_annotation_ok() {
    check_ok!(simple_program(let_(vec![("x", Some(ty("Number")), num("42"))], id("x"))));
}
#[test]
fn let_type_annotation_mismatch() {
    check_err!(simple_program(let_(vec![("x", Some(ty("Boolean")), num("42"))], id("x"))), SemanticError::TypeMismatch { .. });
}
#[test]
fn let_multiple_bindings_sequential_scope() {
    check_ok!(simple_program(let_(vec![("x", None, num("1")), ("y", None, add(id("x"), num("1")))], id("y"))));
}
#[test]
fn let_body_cannot_see_outer_undefined() {
    check_err!(simple_program(let_(vec![("x", None, num("1"))], id("z"))), SemanticError::UndefinedVariable { .. });
}
#[test]
fn let_variable_not_visible_outside() {
    check_err!(simple_program(block(vec![let_(vec![("x", None, num("1"))], id("x")), id("x")])), SemanticError::UndefinedVariable { .. });
}
#[test]
fn let_string_binding_ok() { check_ok!(simple_program(let_(vec![("s", None, str_("hola"))], id("s")))); }

#[test]
fn if_simple_ok() { check_ok!(simple_program(if_(bool_(true), num("1"), num("2")))); }
#[test]
fn if_condition_not_boolean_error() {
    check_err!(simple_program(if_(num("42"), num("1"), num("2"))), SemanticError::TypeMismatch { .. });
}
#[test]
fn if_condition_string_error() {
    check_err!(simple_program(if_(str_("x"), num("1"), num("2"))), SemanticError::TypeMismatch { .. });
}
#[test]
fn if_branches_same_type_ok() { check_ok!(simple_program(if_(bool_(true), num("1"), num("2")))); }
#[test]
fn if_branches_different_types_lca() { check_ok!(simple_program(if_(bool_(true), num("1"), str_("dos")))); }
#[test]
fn if_elif_ok() {
    check_ok!(simple_program(if_elif(bool_(true), num("1"), vec![(bool_(false), num("2"))], num("3"))));
}
#[test]
fn if_elif_condition_not_boolean_error() {
    check_err!(simple_program(if_elif(bool_(true), num("1"), vec![(num("99"), num("2"))], num("3"))), SemanticError::TypeMismatch { .. });
}

#[test]
fn while_ok() { check_ok!(simple_program(while_(bool_(false), num("0")))); }
#[test]
fn while_condition_not_boolean_error() {
    check_err!(simple_program(while_(num("1"), num("0"))), SemanticError::TypeMismatch { .. });
}
#[test]
fn while_body_can_use_outer_scope() {
    check_ok!(simple_program(let_(vec![("x", None, num("0"))], while_(bool_(true), id("x")))));
}

#[test]
fn for_over_range_ok() { check_ok!(simple_program(for_("i", call("range", vec![num("0"), num("10")]), id("i")))); }
#[test]
fn for_over_vector_ok() { check_ok!(simple_program(for_("x", vec_explicit(vec![num("1"), num("2"), num("3")]), id("x")))); }
#[test]
fn for_var_visible_only_in_body() {
    check_err!(simple_program(block(vec![for_("i", call("range", vec![num("0"), num("1")]), id("i")), id("i")])), SemanticError::UndefinedVariable { .. });
}
#[test]
fn for_non_iterable_error() { check_err!(simple_program(for_("x", num("42"), id("x"))), SemanticError::TypeMismatch { .. }); }

#[test]
fn call_builtin_print_ok() { check_ok!(simple_program(call("print", vec![num("42")]))); }
#[test]
fn call_builtin_sqrt_ok() { check_ok!(simple_program(call("sqrt", vec![num("4")]))); }
#[test]
fn call_builtin_range_ok() { check_ok!(simple_program(call("range", vec![num("0"), num("10")]))); }
#[test]
fn call_undefined_function_error() { check_err!(simple_program(call("undefined_fn", vec![])), SemanticError::UndefinedFunction { .. }); }
#[test]
fn call_wrong_arg_count_error() { check_err!(simple_program(call("sqrt", vec![num("1"), num("2")])) , SemanticError::WrongArgCount { .. }); }
#[test]
fn call_wrong_arg_count_zero_error() { check_err!(simple_program(call("print", vec![])), SemanticError::WrongArgCount { .. }); }
#[test]
fn call_user_function_ok() {
    let p = program_with_decls(
        vec![func_decl("double", vec![("x", Some(ty("Number")))], Some(ty("Number")), Expr::binary(BinaryOp::Mul, id("x"), num("2"), d()))],
        call("double", vec![num("5")]),
    );
    check_ok!(p);
}
#[test]
fn call_user_function_wrong_arg_type_error() {
    let p = program_with_decls(
        vec![func_decl("inc", vec![("x", Some(ty("Number")))], Some(ty("Number")), add(id("x"), num("1")))],
        call("inc", vec![str_("hola")]),
    );
    check_err!(p, SemanticError::TypeMismatch { .. });
}
#[test]
fn call_recursive_function_ok() {
    let p = program_with_decls(
        vec![func_decl("fact", vec![("n", Some(ty("Number")))], Some(ty("Number")), if_(eq_(id("n"), num("0")), num("1"), Expr::binary(BinaryOp::Mul, id("n"), call("fact", vec![sub(id("n"), num("1"))]), d())))],
        call("fact", vec![num("5")]),
    );
    check_ok!(p);
}
#[test]
fn function_redefinition_error() {
    let p = program_with_decls(vec![func_decl("foo", vec![], None, num("1")), func_decl("foo", vec![], None, num("2"))], call("foo", vec![]));
    check_err!(p, SemanticError::Redefinition { .. });
}
#[test]
fn function_return_type_mismatch_error() {
    let p = program_with_decls(vec![func_decl("foo", vec![], Some(ty("Boolean")), num("42"))], call("foo", vec![]));
    check_err!(p, SemanticError::TypeMismatch { .. });
}
#[test]
fn function_return_type_ok() {
    let p = program_with_decls(vec![func_decl("get_pi", vec![], Some(ty("Number")), id("PI"))], call("get_pi", vec![]));
    check_ok!(p);
}
#[test]
fn mutually_recursive_functions_ok() {
    let p = program_with_decls(
        vec![
            func_decl("isEven", vec![("n", Some(ty("Number")))], None, if_(eq_(id("n"), num("0")), bool_(true), call("isOdd", vec![sub(id("n"), num("1"))]))),
            func_decl("isOdd", vec![("n", Some(ty("Number")))], None, if_(eq_(id("n"), num("0")), bool_(false), call("isEven", vec![sub(id("n"), num("1"))]))),
        ],
        call("isEven", vec![num("4")]),
    );
    check_ok!(p);
}

#[test]
fn type_simple_ok() {
    let p = program_with_decls(vec![type_decl("Counter", vec![("n", Some(ty("Number")))], None, vec![attr("value", Some(ty("Number")), id("n"))])], new_("Counter", vec![num("0")]));
    check_ok!(p);
}
#[test]
fn type_inherit_ok() {
    let p = program_with_decls(
        vec![type_decl("Animal", vec![("name", Some(ty("String")))], None, vec![attr("name", Some(ty("String")), id("name"))]), type_decl("Dog", vec![("name", Some(ty("String")))], Some("Animal"), vec![])],
        new_("Dog", vec![str_("Rex")]),
    );
    check_ok!(p);
}
#[test]
fn type_inherit_from_number_error() {
    let p = program_with_decls(vec![type_decl("MyNum", vec![], Some("Number"), vec![])], num("0"));
    check_err!(p, SemanticError::InheritFromPrimitive { .. });
}
#[test]
fn type_inherit_from_string_error() {
    let p = program_with_decls(vec![type_decl("MyStr", vec![], Some("String"), vec![])], num("0"));
    check_err!(p, SemanticError::InheritFromPrimitive { .. });
}
#[test]
fn type_inherit_from_boolean_error() {
    let p = program_with_decls(vec![type_decl("MyBool", vec![], Some("Boolean"), vec![])], num("0"));
    check_err!(p, SemanticError::InheritFromPrimitive { .. });
}
#[test]
fn type_self_in_method_ok() {
    let p = program_with_decls(vec![type_decl("Box", vec![("v", Some(ty("Number")))], None, vec![attr("value", Some(ty("Number")), id("v")), method("get", vec![], Some(ty("Number")), access(id("self"), "value"))])], new_("Box", vec![num("1")]));
    check_ok!(p);
}
#[test]
fn type_self_in_initializer_error() {
    let p = program_with_decls(vec![type_decl("Broken", vec![("v", Some(ty("Number")))], None, vec![attr("bad", None, access(id("self"), "something"))])], num("0"));
    check_err!(p, SemanticError::SelfInInitializer { .. });
}
#[test]
fn type_circular_inheritance_error() {
    let p = program_with_decls(vec![type_decl("A", vec![], Some("B"), vec![]), type_decl("B", vec![], Some("A"), vec![])], num("0"));
    check_err!(p, SemanticError::CircularInheritance { .. });
}
#[test]
fn type_new_undefined_error() { check_err!(simple_program(new_("Unicornio", vec![])), SemanticError::UndefinedType { .. }); }
#[test]
fn type_method_with_params_ok() {
    let p = program_with_decls(vec![type_decl("Calc", vec![], None, vec![method("add", vec![("a", Some(ty("Number"))), ("b", Some(ty("Number")))], Some(ty("Number")), add(id("a"), id("b")))])], method_call(new_("Calc", vec![]), "add", vec![num("1"), num("2")])) ;
    check_ok!(p);
}

#[test]
fn protocol_declaration_ok() {
    let p = program_with_decls(vec![protocol_decl("Printable", None, vec![("show", ty("String"))])], num("0"));
    check_ok!(p);
}
#[test]
fn protocol_extends_ok() {
    let p = program_with_decls(vec![protocol_decl("Base", None, vec![("base_method", ty("Number"))]), protocol_decl("Extended", Some("Base"), vec![("extra_method", ty("String"))])], num("0"));
    check_ok!(p);
}
#[test]
fn protocol_extends_undefined_error() {
    let p = program_with_decls(vec![protocol_decl("Derived", Some("NonExistent"), vec![])], num("0"));
    check_err!(p, SemanticError::UndefinedType { .. });
}

#[test]
fn assign_variable_ok() { check_ok!(simple_program(let_(vec![("x", None, num("0"))], assign(id("x"), num("42"))))); }
#[test]
fn assign_type_mismatch_error() {
    check_err!(simple_program(let_(vec![("x", Some(ty("Number")), num("0"))], assign(id("x"), str_("hola")))), SemanticError::TypeMismatch { .. });
}
#[test]
fn assign_to_self_error() {
    let p = program_with_decls(vec![type_decl("T", vec![], None, vec![method("bad", vec![], None, assign(id("self"), num("1")))])], num("0"));
    check_err!(p, SemanticError::SelfAssignment { .. });
}
#[test]
fn assign_invalid_lvalue_error() { check_err!(simple_program(assign(num("42"), num("1"))), SemanticError::InvalidLValue { .. }); }
#[test]
fn assign_invalid_lvalue_call_error() { check_err!(simple_program(assign(call("print", vec![num("1")]), num("1"))), SemanticError::InvalidLValue { .. }); }
#[test]
fn assign_compound_ok() {
    check_ok!(simple_program(let_(vec![("x", None, num("1"))], Expr::assign(AssignOp::PlusAssign, id("x"), num("2"), d()))));
}
#[test]
fn assign_compound_wrong_type_error() {
    check_err!(simple_program(let_(vec![("s", None, str_("hola"))], Expr::assign(AssignOp::PlusAssign, id("s"), num("1"), d()))), SemanticError::InvalidOperandType { .. });
}

#[test]
fn is_expr_ok() { check_ok!(simple_program(is_expr(num("42"), "Number"))); }
#[test]
fn is_undefined_type_error() { check_err!(simple_program(is_expr(num("42"), "Fantasma")), SemanticError::UndefinedType { .. }); }
#[test]
fn as_upcast_ok() {
    let p = program_with_decls(vec![type_decl("Animal", vec![], None, vec![]), type_decl("Dog", vec![], Some("Animal"), vec![])], as_expr(new_("Dog", vec![]), "Animal"));
    check_ok!(p);
}
#[test]
fn as_downcast_ok() {
    let p = program_with_decls(vec![type_decl("Animal", vec![], None, vec![]), type_decl("Dog", vec![], Some("Animal"), vec![])], as_expr(new_("Animal", vec![]), "Dog"));
    check_ok!(p);
}
#[test]
fn as_unrelated_types_error() {
    let p = program_with_decls(vec![type_decl("Cat", vec![], None, vec![]), type_decl("Dog", vec![], None, vec![])], as_expr(new_("Cat", vec![]), "Dog"));
    check_err!(p, SemanticError::InvalidCast { .. });
}

#[test]
fn vector_explicit_homogeneous_ok() { check_ok!(simple_program(vec_explicit(vec![num("1"), num("2"), num("3")]))); }
#[test]
fn vector_explicit_empty_ok() { check_ok!(simple_program(vec_explicit(vec![]))); }
#[test]
fn vector_explicit_mixed_types_ok() { check_ok!(simple_program(vec_explicit(vec![num("1"), str_("dos")]))); }
#[test]
fn vector_generator_ok() {
    check_ok!(simple_program(vec_gen(Expr::binary(BinaryOp::Mul, id("x"), num("2"), d()), "x", call("range", vec![num("0"), num("5")]))));
}
#[test]
fn vector_generator_var_scoped_ok() {
    check_err!(simple_program(block(vec![vec_gen(id("x"), "x", call("range", vec![num("0"), num("1")])), id("x")])), SemanticError::UndefinedVariable { .. });
}
#[test]
fn vector_index_ok() {
    check_ok!(simple_program(let_(vec![("v", None, vec_explicit(vec![num("1"), num("2"), num("3")]))], Expr::Index(Box::new(crate::parser::ast::IndexExpr::new(id("v"), num("0"), d()))))));
}
#[test]
fn vector_index_non_number_error() {
    check_err!(simple_program(let_(vec![("v", None, vec_explicit(vec![num("1")]))], Expr::Index(Box::new(crate::parser::ast::IndexExpr::new(id("v"), str_("cero"), d()))))), SemanticError::TypeMismatch { .. });
}
#[test]
fn index_non_vector_error() {
    check_err!(simple_program(let_(vec![("x", None, num("5"))], Expr::Index(Box::new(crate::parser::ast::IndexExpr::new(id("x"), num("0"), d()))))), SemanticError::TypeMismatch { .. });
}

#[test]
fn method_call_ok() {
    let p = program_with_decls(vec![type_decl("Greeter", vec![], None, vec![method("greet", vec![], Some(ty("String")), str_("hola"))])], method_call(new_("Greeter", vec![]), "greet", vec![]));
    check_ok!(p);
}
#[test]
fn method_not_found_error() {
    let p = program_with_decls(vec![type_decl("Empty", vec![], None, vec![])], method_call(new_("Empty", vec![]), "nonexistent", vec![]));
    check_err!(p, SemanticError::MethodNotFound { .. });
}
#[test]
fn attribute_access_not_found_error() {
    let p = program_with_decls(vec![type_decl("Point", vec![], None, vec![])], access(new_("Point", vec![]), "z"));
    check_err!(p, SemanticError::AttributeNotFound { .. });
}
#[test]
fn inherited_method_accessible_ok() {
    let p = program_with_decls(vec![type_decl("A", vec![], None, vec![method("foo", vec![], Some(ty("Number")), num("1"))]), type_decl("B", vec![], Some("A"), vec![])], method_call(new_("B", vec![]), "foo", vec![]));
    check_ok!(p);
}
#[test]
fn builtin_string_size_ok() { check_ok!(simple_program(method_call(str_("hola"), "size", vec![]))); }
#[test]
fn builtin_tostring_ok() { check_ok!(simple_program(method_call(num("42"), "toString", vec![]))); }

#[test]
fn base_in_method_ok() {
    let p = program_with_decls(
        vec![
            type_decl(
                "A",
                vec![],
                None,
                vec![method("val", vec![], Some(ty("Number")), num("1"))],
            ),
            type_decl(
                "B",
                vec![],
                Some("A"),
                vec![method(
                    "val",
                    vec![],
                    Some(ty("Number")),
                    method_call(Expr::Base(d()), "val", vec![]),
                )],
            ),
        ],
        method_call(new_("B", vec![]), "val", vec![]),
    );

    check_ok!(p);
}


#[test]
fn base_outside_type_error() { check_err!(simple_program(Expr::Base(d())), SemanticError::UndefinedVariable { .. }); }

#[test]
fn never_does_not_cascade_in_binary() {
    let errors = match analyze(&simple_program(add(id("undefined_var"), num("1")))) {
        Ok(()) => vec![], Err(errors) => errors,
    };
    assert_eq!(errors.iter().filter(|e| matches!(e, SemanticError::UndefinedVariable { .. })).count(), 1, "Debe haber exactamente 1 UndefinedVariable");
    assert!(!errors.iter().any(|e| matches!(e, SemanticError::InvalidBinaryTypes { .. })), "No debe haber InvalidBinaryTypes cuando un operando es Never");
}
#[test]
fn never_does_not_cascade_in_if() {
    let count = error_count!(simple_program(if_(id("undefined_cond"), num("1"), num("2"))));
    assert_eq!(count, 1, "Solo debe haber 1 error (UndefinedVariable)");
}

#[test]
fn complete_fibonacci_ok() {
    let p = program_with_decls(vec![func_decl("fib", vec![("n", Some(ty("Number")))], Some(ty("Number")), if_elif(eq_(id("n"), num("0")), num("0"), vec![(eq_(id("n"), num("1")), num("1"))], add(call("fib", vec![sub(id("n"), num("1"))]), call("fib", vec![sub(id("n"), num("2"))]))))], call("print", vec![call("fib", vec![num("10")])]));
    check_ok!(p);
}
#[test]
fn complete_point_type_ok() {
    let p = program_with_decls(
        vec![type_decl("Point", vec![("x", Some(ty("Number"))), ("y", Some(ty("Number")))], None, vec![attr("x", Some(ty("Number")), id("x")), attr("y", Some(ty("Number")), id("y")), method("dist", vec![], Some(ty("Number")), call("sqrt", vec![add(Expr::binary(BinaryOp::Mul, access(id("self"), "x"), access(id("self"), "x"), d()), Expr::binary(BinaryOp::Mul, access(id("self"), "y"), access(id("self"), "y"), d()))]))])],
        let_(vec![("p", None, new_("Point", vec![num("3"), num("4")]))], method_call(id("p"), "dist", vec![])),
    );
    check_ok!(p);
}
#[test]
fn complete_vector_sum_ok() {
    let p = simple_program(let_(vec![("v", None, vec_explicit(vec![num("1"), num("2"), num("3")]))], let_(vec![("total", None, num("0"))], block(vec![for_("x", id("v"), assign(id("total"), add(id("total"), id("x")))), id("total")]))));
    check_ok!(p);
}

#[test]
fn constructor_correct_arity_ok() {
    let p = program_with_decls(vec![type_decl("Point", vec![("x", Some(ty("Number"))), ("y", Some(ty("Number")))], None, vec![attr("x", Some(ty("Number")), id("x")), attr("y", Some(ty("Number")), id("y"))])], new_("Point", vec![num("1"), num("2")]));
    check_ok!(p);
}
#[test]
fn constructor_too_few_args_error() {
    let p = program_with_decls(vec![type_decl("Point", vec![("x", Some(ty("Number"))), ("y", Some(ty("Number")))], None, vec![])], new_("Point", vec![num("1")]));
    check_err!(p, SemanticError::WrongArgCount { .. });
}
#[test]
fn constructor_too_many_args_error() {
    let p = program_with_decls(vec![type_decl("Point", vec![("x", Some(ty("Number"))), ("y", Some(ty("Number")))], None, vec![])], new_("Point", vec![num("1"), num("2"), num("3")]));
    check_err!(p, SemanticError::WrongArgCount { .. });
}
#[test]
fn constructor_wrong_arg_type_error() {
    let p = program_with_decls(vec![type_decl("Point", vec![("x", Some(ty("Number"))), ("y", Some(ty("Number")))], None, vec![])], new_("Point", vec![num("1"), str_("dos")]));
    check_err!(p, SemanticError::TypeMismatch { .. });
}
#[test]
fn constructor_no_args_ok() {
    let p = program_with_decls(vec![type_decl("Empty", vec![], None, vec![])], new_("Empty", vec![]));
    check_ok!(p);
}
#[test]
fn constructor_no_args_with_args_error() {
    let p = program_with_decls(vec![type_decl("Empty", vec![], None, vec![])], new_("Empty", vec![num("1")]));
    check_err!(p, SemanticError::WrongArgCount { .. });
}

#[test]
fn override_same_signature_ok() {
    let p = program_with_decls(
        vec![
            type_decl("A", vec![], None, vec![method("foo", vec![("x", Some(ty("Number")))], Some(ty("Number")), id("x"))]),
            type_decl("B", vec![], Some("A"), vec![method("foo", vec![("x", Some(ty("Number")))], Some(ty("Number")), add(id("x"), num("1")))]),
        ],
        num("0"),
    );
    check_ok!(p);
}
#[test]
fn override_different_param_count_error() {
    let p = program_with_decls(
        vec![
            type_decl("A", vec![], None, vec![method("foo", vec![("x", Some(ty("Number")))], Some(ty("Number")), id("x"))]),
            type_decl("B", vec![], Some("A"), vec![method("foo", vec![("x", Some(ty("Number"))), ("y", Some(ty("Number")))], Some(ty("Number")), id("x"))]),
        ],
        num("0"),
    );
    check_err!(p, SemanticError::OverrideMismatch { .. });
}
#[test]
fn override_different_param_type_error() {
    let p = program_with_decls(
        vec![
            type_decl("A", vec![], None, vec![method("foo", vec![("x", Some(ty("Number")))], Some(ty("Number")), id("x"))]),
            type_decl("B", vec![], Some("A"), vec![method("foo", vec![("x", Some(ty("String")))], Some(ty("Number")), num("0"))]),
        ],
        num("0"),
    );
    check_err!(p, SemanticError::OverrideMismatch { .. });
}
#[test]
fn override_incompatible_return_type_error() {
    let p = program_with_decls(
        vec![
            type_decl("A", vec![], None, vec![method("foo", vec![], Some(ty("Number")), num("1"))]),
            type_decl("B", vec![], Some("A"), vec![method("foo", vec![], Some(ty("String")), str_("x"))]),
        ],
        num("0"),
    );
    check_err!(p, SemanticError::OverrideMismatch { .. });
}

#[test]
fn base_call_in_constructor_ok() {
    let p = program_with_decls(
        vec![
            type_decl("Animal", vec![("name", Some(ty("String")))], None, vec![attr("name", Some(ty("String")), id("name"))]),
            type_decl("Dog", vec![("name", Some(ty("String")))], Some("Animal"), vec![method("init", vec![], None, Expr::Call(Box::new(crate::parser::ast::CallExpr::new(Expr::Base(d()), vec![id("name")], d()))))]),
        ],
        new_("Dog", vec![str_("Rex")]),
    );
    check_ok!(p);
}
#[test]
fn base_call_wrong_arg_count_error() {
    let p = program_with_decls(
        vec![
            type_decl("Animal", vec![("name", Some(ty("String")))], None, vec![attr("name", Some(ty("String")), id("name"))]),
            type_decl("Dog", vec![("name", Some(ty("String")))], Some("Animal"), vec![method("bad_init", vec![], None, Expr::Call(Box::new(crate::parser::ast::CallExpr::new(Expr::Base(d()), vec![], d()))))]),
        ],
        num("0"),
    );
    check_err!(p, SemanticError::WrongArgCount { .. });
}
#[test]
fn base_call_outside_type_error() {
    check_err!(simple_program(Expr::Call(Box::new(crate::parser::ast::CallExpr::new(Expr::Base(d()), vec![], d())))), SemanticError::UndefinedVariable { .. });
}

#[test]
fn vector_annotation_correct_ok() {
    check_ok!(simple_program(let_(vec![("v", Some(TypeName::vector("Number", d())), vec_explicit(vec![num("1"), num("2"), num("3")]))], id("v"))));
}
#[test]
fn vector_annotation_covariant_ok() {
    check_ok!(simple_program(let_(vec![("v", Some(TypeName::vector("Object", d())), vec_explicit(vec![num("1"), num("2"), num("3")]))], id("v"))));
}
#[test]
fn vector_annotation_type_mismatch_error() {
    check_err!(simple_program(let_(vec![("v", Some(TypeName::vector("Number", d())), vec_explicit(vec![str_("a"), str_("b")]))], id("v"))), SemanticError::TypeMismatch { .. });
}
#[test]
fn vector_annotation_bool_mismatch_error() {
    check_err!(simple_program(let_(vec![("v", Some(TypeName::vector("Boolean", d())), vec_explicit(vec![num("1"), num("2")]))], id("v"))), SemanticError::TypeMismatch { .. });
}

#[test]
fn infer_return_type_used_in_arithmetic_ok() {
    let p = program_with_decls(vec![func_decl("double", vec![("x", Some(ty("Number")))], None, Expr::binary(BinaryOp::Mul, id("x"), num("2"), d()))], let_(vec![("r", Some(ty("Number")), call("double", vec![num("5")]))], id("r")));
    check_ok!(p);
}
#[test]
fn infer_return_type_mismatch_error() {
    let p = program_with_decls(vec![func_decl("get_name", vec![], None, str_("hulk"))], let_(vec![("n", Some(ty("Number")), call("get_name", vec![]))], id("n")));
    check_err!(p, SemanticError::TypeMismatch { .. });
}
#[test]
fn infer_return_type_propagates_to_call_ok() {
    let p = program_with_decls(vec![func_decl("square", vec![("x", Some(ty("Number")))], None, Expr::binary(BinaryOp::Power, id("x"), num("2"), d()))], add(call("square", vec![num("3")]), num("1")));
    check_ok!(p);
}

#[test]
fn complete_multiple_errors_detected() {
    let p = program_with_decls(vec![func_decl("foo", vec![], None, id("undefined_in_foo")), func_decl("foo", vec![], None, num("1"))], add(str_("x"), bool_(true)));
    let errors = match analyze(&p) { Ok(()) => vec![], Err(errors) => errors };
    assert!(errors.len() >= 3, "Se esperaban al menos 3 errores, hubo {}:\n{}", errors.len(), errors.iter().map(|e| format!("  • {}", e)).collect::<Vec<_>>().join("\n"));
}
