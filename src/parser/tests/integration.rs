// src/parser/tests/integration.rs
//
// Tests de integración del parser HULK.
// Cada test parsea código HULK real y verifica la forma exacta del AST.
//
// Cómo añadir un test nuevo:
//   1. Escribe el código HULK como string.
//   2. Llama a `parse(src)` para obtener el Program.
//   3. Usa las funciones de inspección (as_call, as_binary, as_let, …)
//      para navegar el árbol sin boilerplate.

use crate::lexer::lexer::Lexer;
use crate::lexer::master_nfa::MasterNFA;
use crate::lexer::token_definition::TokenDefinition;
use crate::parser::ast::*;
use crate::parser::engine::global_driver;

// ─────────────────────────────────────────────────────────────────────────────
//  Utilidades de test
// ─────────────────────────────────────────────────────────────────────────────

/// Tokeniza un string usando el lexer real del proyecto.
fn tokenize(src: &str) -> Vec<crate::lexer::token::Token> {
    let defs = TokenDefinition::default_token_definitions();
    let master = MasterNFA::from_token_definitions(&defs);
    let mut lexer = Lexer::new(src, master);
    lexer.tokenize()
}

/// Parsea un string de código HULK y devuelve el Program.
/// Hace panic con el error si el parse falla, mostrando el mensaje completo.
fn parse(src: &str) -> Program {
    let tokens = tokenize(src);

    global_driver()
        .parse(tokens.into_iter())
        .unwrap_or_else(|e| panic!("Parse falló:\n  código: {}\n  error:  {}", src, e))
}

/// Igual que parse() pero espera un error y devuelve su mensaje.
fn parse_err(src: &str) -> String {
    let tokens = tokenize(src);
    global_driver()
        .parse(tokens.into_iter())
        .expect_err(&format!("Se esperaba error pero parseó bien: {}", src))
        .to_string()
}

// ── Navegadores del AST ───────────────────────────────────────────────────────

/// Extrae el Expr de la entrada del programa. Panic si no es Expr.
fn entry(program: &Program) -> &Expr {
    program.entry.as_ref()
}

/// Extrae el nombre y args de una llamada. Panic si no es Call.
fn as_call<'a>(expr: &'a Expr) -> (&'a Expr, &'a [Expr]) {
    match &expr.kind {
        ExprKind::Call(c) => (&c.callee, &c.args),
        other => panic!(
            "esperaba Call, encontrado {:?}",
            std::mem::discriminant(other)
        ),
    }
}

/// Extrae op, left, right de una expresión binaria.
fn as_binary(expr: &Expr) -> (&BinaryOp, &Expr, &Expr) {
    match &expr.kind {
        ExprKind::Binary(b) => (&b.op, &b.left, &b.right),
        other => panic!(
            "esperaba Binary, encontrado {:?}",
            std::mem::discriminant(other)
        ),
    }
}

/// Extrae el nombre de un Identifier.
fn as_ident(expr: &Expr) -> &str {
    match &expr.kind {
        ExprKind::Identifier { name } => name,
        other => panic!(
            "esperaba Identifier, encontrado {:?}",
            std::mem::discriminant(other)
        ),
    }
}

/// Extrae el valor string de un Number literal.
fn as_number(expr: &Expr) -> &str {
    match &expr.kind {
        ExprKind::Literal(Literal::Number { value, .. }) => value,
        other => panic!(
            "esperaba Number, encontrado {:?}",
            std::mem::discriminant(other)
        ),
    }
}

/// Extrae el valor string de un String literal.
fn as_string_lit(expr: &Expr) -> &str {
    match &expr.kind {
        ExprKind::Literal(Literal::String { value, .. }) => value,
        other => panic!(
            "esperaba String literal, encontrado {:?}",
            std::mem::discriminant(other)
        ),
    }
}

/// Extrae el bool de un Bool literal.
fn as_bool(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Literal(Literal::Bool { value, .. }) => *value,
        other => panic!(
            "esperaba Bool, encontrado {:?}",
            std::mem::discriminant(other)
        ),
    }
}

/// Extrae bindings y body de un LetExpr.
fn as_let(expr: &Expr) -> (&[LetBinding], &Expr) {
    match &expr.kind {
        ExprKind::Let(l) => (&l.bindings, &l.body),
        other => panic!(
            "esperaba Let, encontrado {:?}",
            std::mem::discriminant(other)
        ),
    }
}

/// Extrae condition, then, elifs, else de un IfExpr.
fn as_if(expr: &Expr) -> (&Expr, &Expr, &[ElifBranch], &Expr) {
    match &expr.kind {
        ExprKind::If(i) => (&i.condition, &i.then_body, &i.elif_chain, &i.else_body),
        other => panic!(
            "esperaba If, encontrado {:?}",
            std::mem::discriminant(other)
        ),
    }
}

/// Extrae condition y body de un WhileExpr.
fn as_while(expr: &Expr) -> (&Expr, &Expr) {
    match &expr.kind {
        ExprKind::While(w) => (&w.condition, &w.body),
        other => panic!(
            "esperaba While, encontrado {:?}",
            std::mem::discriminant(other)
        ),
    }
}

/// Extrae var, iterable, body de un ForExpr.
fn as_for(expr: &Expr) -> (&str, &Expr, &Expr) {
    match &expr.kind {
        ExprKind::For(f) => (&f.var, &f.iterable, &f.body),
        other => panic!(
            "esperaba For, encontrado {:?}",
            std::mem::discriminant(other)
        ),
    }
}

/// Extrae el body de un BlockExpr.
fn as_block(expr: &Expr) -> &[Expr] {
    match &expr.kind {
        ExprKind::Block(b) => &b.body,
        other => panic!(
            "esperaba Block, encontrado {:?}",
            std::mem::discriminant(other)
        ),
    }
}

/// Extrae target, op, value de una AssignExpr.
fn as_assign(expr: &Expr) -> (&Expr, &AssignOp, &Expr) {
    match &expr.kind {
        ExprKind::Assign(a) => (&a.target, &a.op, &a.value),
        other => panic!(
            "esperaba Assign, encontrado {:?}",
            std::mem::discriminant(other)
        ),
    }
}

/// Extrae object y field de un AccessExpr.
fn as_access<'a>(expr: &'a Expr) -> (&'a Expr, &'a str) {
    match &expr.kind {
        ExprKind::Access(a) => (&a.object, &a.field),
        other => panic!(
            "esperaba Access, encontrado {:?}",
            std::mem::discriminant(other)
        ),
    }
}

/// Extrae object, method, args de un MethodCallExpr.
fn as_method_call<'a>(expr: &'a Expr) -> (&'a Expr, &'a str, &'a [Expr]) {
    match &expr.kind {
        ExprKind::MethodCall(m) => (&m.object, &m.method, &m.args),
        other => panic!(
            "esperaba MethodCall, encontrado {:?}",
            std::mem::discriminant(other)
        ),
    }
}

/// Extrae la primera declaración como FuncDecl.
fn first_func(program: &Program) -> &FuncDecl {
    match program.declarations.first() {
        Some(Decl::Function(f)) => f,
        other => panic!(
            "primera declaración no es Function: {:?}",
            other.map(|d| d.name())
        ),
    }
}

/// Extrae la primera declaración como TypeDecl.
fn first_type(program: &Program) -> &TypeDecl {
    match program.declarations.first() {
        Some(Decl::Type(t)) => t,
        other => panic!(
            "primera declaración no es Type: {:?}",
            other.map(|d| d.name())
        ),
    }
}

/// Extrae la primera declaración como ProtocolDecl.
fn first_protocol(program: &Program) -> &ProtocolDecl {
    match program.declarations.first() {
        Some(Decl::Protocol(p)) => p,
        other => panic!(
            "primera declaración no es Protocol: {:?}",
            other.map(|d| d.name())
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tests — Expresiones simples
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn literal_number() {
    let p = parse("42;");
    assert_eq!(as_number(entry(&p)), "42");
}

#[test]
fn literal_string() {
    let p = parse(r#""hello world";"#);
    assert_eq!(as_string_lit(entry(&p)), "\"hello world\"");
}

#[test]
fn literal_bool_true() {
    let p = parse("true;");
    assert!(as_bool(entry(&p)));
}

#[test]
fn literal_bool_false() {
    let p = parse("false;");
    assert!(!as_bool(entry(&p)));
}

#[test]
fn identifier_expr() {
    let p = parse("myVar;");
    assert_eq!(as_ident(entry(&p)), "myVar");
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tests — Aritmética y operadores
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn addition() {
    let p = parse("1 + 2;");
    let (op, left, right) = as_binary(entry(&p));
    assert_eq!(*op, BinaryOp::Add);
    assert_eq!(as_number(left), "1");
    assert_eq!(as_number(right), "2");
}

#[test]
fn operator_precedence_mul_over_add() {
    // 1 + 2 * 3  debe ser  1 + (2 * 3)
    let p = parse("1 + 2 * 3;");
    let (op, left, right) = as_binary(entry(&p));
    assert_eq!(*op, BinaryOp::Add);
    assert_eq!(as_number(left), "1");
    // right debe ser (2 * 3)
    let (op2, l2, r2) = as_binary(right);
    assert_eq!(*op2, BinaryOp::Mul);
    assert_eq!(as_number(l2), "2");
    assert_eq!(as_number(r2), "3");
}

#[test]
fn power_right_associative() {
    // 2 ^ 3 ^ 4  debe ser  2 ^ (3 ^ 4)
    let p = parse("2 ^ 3 ^ 4;");
    let (op, left, right) = as_binary(entry(&p));
    assert_eq!(*op, BinaryOp::Power);
    assert_eq!(as_number(left), "2");
    let (op2, l2, r2) = as_binary(right);
    assert_eq!(*op2, BinaryOp::Power);
    assert_eq!(as_number(l2), "3");
    assert_eq!(as_number(r2), "4");
}

#[test]
fn unary_negation() {
    let p = parse("-42;");
    match entry(&p) {
        ExprKind::Unary(u) => {
            assert_eq!(u.op, UnaryOp::Neg);
            assert_eq!(as_number(&u.operand), "42");
        }
        other => panic!("esperaba Unary, got {:?}", std::mem::discriminant(other)),
    }
}

#[test]
fn string_concatenation() {
    // Si falla aquí con "esperaba Binary, encontrado Literal",
    // verificar que el lexer tokenice '@' como OP_CONCAT (no ERROR).
    // Debug: ejecutar con --nocapture y revisar tokens del lexer.
    let tokens = tokenize(r#""hello" @ " world";"#);
    println!(
        "Tokens para '@': {:?}",
        tokens.iter().map(|t| &t.token_type).collect::<Vec<_>>()
    );
    let p = parse(r#""hello" @ " world";"#);
    let (op, _, _) = as_binary(entry(&p));
    assert_eq!(*op, BinaryOp::Concat);
}

#[test]
fn double_concat() {
    let tokens = tokenize(r#""hello" @@ "world";"#);
    for token in tokens {
        println!("{:?}", token);
    }
    let p = parse(r#""hello" @@ "world";"#);
    let (op, _, _) = as_binary(entry(&p));
    assert_eq!(*op, BinaryOp::DoubleConcat);
}

#[test]
fn comparison_equal() {
    let p = parse("x == 0;");
    let (op, left, right) = as_binary(entry(&p));
    assert_eq!(*op, BinaryOp::Eq);
    assert_eq!(as_ident(left), "x");
    assert_eq!(as_number(right), "0");
}

#[test]
fn logical_and_or() {
    // true & false | true  →  (true & false) | true  (& tiene mayor prec que |)
    let p = parse("true & false | true;");
    let (op, left, _) = as_binary(entry(&p));
    assert_eq!(*op, BinaryOp::Or);
    let (op2, _, _) = as_binary(left);
    assert_eq!(*op2, BinaryOp::And);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tests — Llamadas a función
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn call_no_args() {
    let p = parse("rand();");
    let (callee, args) = as_call(entry(&p));
    assert_eq!(as_ident(callee), "rand");
    assert!(args.is_empty());
}

#[test]
fn call_one_arg() {
    let p = parse("print(42);");
    let (callee, args) = as_call(entry(&p));
    assert_eq!(as_ident(callee), "print");
    assert_eq!(args.len(), 1);
    assert_eq!(as_number(&args[0]), "42");
}

#[test]
fn call_multiple_args() {
    let p = parse("log(2, 8);");
    let (callee, args) = as_call(entry(&p));
    assert_eq!(as_ident(callee), "log");
    assert_eq!(args.len(), 2);
    assert_eq!(as_number(&args[0]), "2");
    assert_eq!(as_number(&args[1]), "8");
}

#[test]
fn nested_call() {
    // print(sqrt(2))
    let p = parse("print(sqrt(2));");
    let (callee, args) = as_call(entry(&p));
    assert_eq!(as_ident(callee), "print");
    let (inner_callee, inner_args) = as_call(&args[0]);
    assert_eq!(as_ident(inner_callee), "sqrt");
    assert_eq!(as_number(&inner_args[0]), "2");
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tests — Let
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn let_single_binding() {
    let p = parse("let x = 42 in print(x);");
    let (bindings, body) = as_let(entry(&p));
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].name, "x");
    assert!(bindings[0].type_ann.is_none());
    assert_eq!(as_number(&bindings[0].value), "42");
    let (callee, args) = as_call(body);
    assert_eq!(as_ident(callee), "print");
    assert_eq!(as_ident(&args[0]), "x");
}

#[test]
fn let_with_type_annotation() {
    let p = parse("let x: Number = 42 in x;");
    let (bindings, _) = as_let(entry(&p));
    assert_eq!(bindings[0].name, "x");
    let ann = bindings[0].type_ann.as_ref().unwrap();
    assert_eq!(ann.name(), "Number");
}

#[test]
fn let_multiple_bindings() {
    let p = parse("let a = 1, b = 2 in a + b;");
    let (bindings, body) = as_let(entry(&p));
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0].name, "a");
    assert_eq!(bindings[1].name, "b");
    let (op, _, _) = as_binary(body);
    assert_eq!(*op, BinaryOp::Add);
}

#[test]
fn let_nested() {
    // let a = (let b = 6 in b * 7) in print(a)
    let p = parse("let a = (let b = 6 in b * 7) in print(a);");
    let (bindings, _) = as_let(entry(&p));
    assert_eq!(bindings[0].name, "a");
    // El valor del binding es otro let
    let (inner_bindings, inner_body) = as_let(&bindings[0].value);
    assert_eq!(inner_bindings[0].name, "b");
    let (op, _, _) = as_binary(inner_body);
    assert_eq!(*op, BinaryOp::Mul);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tests — If / Elif / Else
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn if_simple() {
    let p = parse("if (x > 0) 1 else 0;");
    let (cond, then, elifs, else_) = as_if(entry(&p));
    let (op, left, right) = as_binary(cond);
    assert_eq!(*op, BinaryOp::Greater);
    assert_eq!(as_ident(left), "x");
    assert_eq!(as_number(right), "0");
    assert_eq!(as_number(then), "1");
    assert!(elifs.is_empty());
    assert_eq!(as_number(else_), "0");
}

#[test]
fn if_elif_else() {
    let p = parse(
        r#"
        if (x > 0) "positive"
        elif (x < 0) "negative"
        else "zero";
    "#,
    );
    let (_, then, elifs, else_) = as_if(entry(&p));
    assert_eq!(as_string_lit(then), "\"positive\"");
    assert_eq!(elifs.len(), 1);
    let (elif_op, _, _) = as_binary(&elifs[0].condition);
    assert_eq!(*elif_op, BinaryOp::Less);
    assert_eq!(as_string_lit(&elifs[0].body), "\"negative\"");
    assert_eq!(as_string_lit(else_), "\"zero\"");
}

#[test]
fn if_with_block_body() {
    let p = parse(
        r#"
        if (true) {
            print(1);
            print(2)
        } else 0;
    "#,
    );
    let (_, then, _, _) = as_if(entry(&p));
    let stmts = as_block(then);
    assert_eq!(stmts.len(), 2);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tests — While / For
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn while_loop() {
    let p = parse("while (x >= 0) x := x - 1;");
    let (cond, body) = as_while(entry(&p));
    let (op, _, _) = as_binary(cond);
    assert_eq!(*op, BinaryOp::GreaterEq);
    let (_, aop, _) = as_assign(body);
    assert_eq!(*aop, AssignOp::Assign);
}

#[test]
fn for_loop() {
    let p = parse("for (x in range(0, 10)) print(x);");
    let (var, iterable, body) = as_for(entry(&p));
    assert_eq!(var, "x");
    let (callee, args) = as_call(iterable);
    assert_eq!(as_ident(callee), "range");
    assert_eq!(args.len(), 2);
    let (print_callee, _) = as_call(body);
    assert_eq!(as_ident(print_callee), "print");
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tests — Bloques
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn block_multiple_stmts() {
    let p = parse("{ print(1); print(2); 42 }");
    let stmts = as_block(entry(&p));
    assert_eq!(stmts.len(), 3);
    assert_eq!(as_number(&stmts[2]), "42");
}

#[test]
fn block_is_expression() {
    // El valor del bloque es la última expresión
    let p = parse("let x = { 1; 2; 3 } in x;");
    let (bindings, _) = as_let(entry(&p));
    let stmts = as_block(&bindings[0].value);
    assert_eq!(stmts.len(), 3);
    assert_eq!(as_number(&stmts[2]), "3");
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tests — Acceso a miembros
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn member_access() {
    let p = parse("pt.x;");
    let (object, field) = as_access(entry(&p));
    assert_eq!(as_ident(object), "pt");
    assert_eq!(field, "x");
}

#[test]
fn method_call() {
    let p = parse("pt.getX();");
    let (object, method, args) = as_method_call(entry(&p));
    assert_eq!(as_ident(object), "pt");
    assert_eq!(method, "getX");
    assert!(args.is_empty());
}

#[test]
fn chained_method_calls() {
    // pt.setX(3).getX()
    let p = parse(r#"pt.setX(3).getX();"#);
    println!("{:?}", p);
    let (object, method, args) = as_method_call(entry(&p));
    assert_eq!(method, "getX");
    assert!(args.is_empty());
    // object debe ser pt.setX(3)
    let (inner_obj, inner_method, inner_args) = as_method_call(object);
    assert_eq!(as_ident(inner_obj), "pt");
    assert_eq!(inner_method, "setX");
    assert_eq!(as_number(&inner_args[0]), "3");
}

#[test]
fn index_access() {
    let p = parse("v[0];");
    match entry(&p) {
        ExprKind::Index(i) => {
            assert_eq!(as_ident(&i.collection), "v");
            assert_eq!(as_number(&i.index), "0");
        }
        other => panic!("esperaba Index: {:?}", std::mem::discriminant(other)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tests — new
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn new_no_args() {
    let p = parse("new Point();");
    match entry(&p) {
        ExprKind::New(n) => {
            assert_eq!(n.type_name.name(), "Point");
            assert!(n.args.is_empty());
        }
        other => panic!("esperaba New: {:?}", std::mem::discriminant(other)),
    }
}

#[test]
fn new_with_args() {
    let p = parse("new Point(3, 4);");
    match entry(&p) {
        ExprKind::New(n) => {
            assert_eq!(n.type_name.name(), "Point");
            assert_eq!(n.args.len(), 2);
            assert_eq!(as_number(&n.args[0]), "3");
            assert_eq!(as_number(&n.args[1]), "4");
        }
        other => panic!("esperaba New: {:?}", std::mem::discriminant(other)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tests — Vectores
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn vector_empty() {
    let p = parse("[];");
    match entry(&p) {
        ExprKind::Vector(v) => match v.as_ref() {
            VectorExpr::Explicit { elements, .. } => assert!(elements.is_empty()),
            _ => panic!("esperaba Explicit"),
        },
        other => panic!("esperaba Vector: {:?}", std::mem::discriminant(other)),
    }
}

#[test]
fn vector_explicit() {
    let p = parse("[1, 2, 3];");
    match entry(&p) {
        ExprKind::Vector(v) => match v.as_ref() {
            VectorExpr::Explicit { elements, .. } => {
                assert_eq!(elements.len(), 3);
                assert_eq!(as_number(&elements[0]), "1");
                assert_eq!(as_number(&elements[2]), "3");
            }
            _ => panic!("esperaba Explicit"),
        },
        other => panic!("esperaba Vector: {:?}", std::mem::discriminant(other)),
    }
}

// ⚠ vector_generator está deshabilitado: conflicto shift/reduce conocido en la gramática.
// El token '|' es ambiguo entre OR lógico y separador de generador.
// Se resolverá en table_builder.rs con desambiguación contextual.
// #[test]
// fn vector_generator() { ... }

// ─────────────────────────────────────────────────────────────────────────────
//  Tests — Declaración de funciones
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn func_inline_no_type() {
    let p = parse("function double(x) => x * 2; 0;");
    let f = first_func(&p);
    assert_eq!(f.name, "double");
    assert_eq!(f.params.len(), 1);
    assert_eq!(f.params[0].name, "x");
    assert!(f.return_type.is_none());
    let (op, left, right) = as_binary(&f.body);
    assert_eq!(*op, BinaryOp::Mul);
    assert_eq!(as_ident(left), "x");
    assert_eq!(as_number(right), "2");
}

#[test]
fn func_inline_with_return_type() {
    let p = parse("function id(x: Number): Number => x; 0;");
    println!("{:?}", p);
    let f = first_func(&p);
    assert_eq!(f.params[0].type_ann.as_ref().unwrap().name(), "Number");
    assert_eq!(f.return_type.as_ref().unwrap().name(), "Number");
}

#[test]
fn func_block_body() {
    let p = parse("function operate(x, y) { print(x + y); print(x - y) } 0;");
    let f = first_func(&p);
    assert_eq!(f.name, "operate");
    assert_eq!(f.params.len(), 2);
    let stmts = as_block(&f.body);
    assert_eq!(stmts.len(), 2);
}

#[test]
fn func_recursive() {
    let p = parse("function fib(n) => if (n == 0 | n == 1) 1 else fib(n-1) + fib(n-2); 0;");
    let f = first_func(&p);
    assert_eq!(f.name, "fib");
    assert!(matches!(f.body.kind, ExprKind::If(_)));
}

#[test]
fn multiple_func_decls() {
    let p = parse("function f(x) => x; function g(x) => x * 2; f(g(3));");
    assert_eq!(p.declarations.len(), 2);
    assert_eq!(p.declarations[0].name(), "f");
    assert_eq!(p.declarations[1].name(), "g");
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tests — Declaración de tipos
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn type_no_args_no_inherit() {
    let p = parse("type Counter { count = 0; } new Counter();");
    let t = first_type(&p);
    assert_eq!(t.name, "Counter");
    assert!(t.type_args.is_empty());
    assert!(t.parent.is_none());
    assert_eq!(t.members.len(), 1);
    match &t.members[0] {
        TypeMember::Attribute(a) => {
            assert_eq!(a.name, "count");
            assert_eq!(as_number(&a.value), "0");
        }
        _ => panic!("esperaba Attribute"),
    }
}

#[test]
fn type_with_args() {
    let p = parse("type Point(x, y) { x = x; y = y; } new Point(0,0);");
    let t = first_type(&p);
    assert_eq!(t.type_args.len(), 2);
    assert_eq!(t.type_args[0].name, "x");
    assert_eq!(t.type_args[1].name, "y");
}

#[test]
fn type_with_method() {
    let p = parse("type Point(x, y) { x = x; getX() => self.x; } new Point(0,0);");
    let t = first_type(&p);
    let method = t
        .members
        .iter()
        .find_map(|m| {
            if let TypeMember::Method(meth) = m {
                Some(meth)
            } else {
                None
            }
        })
        .expect("no se encontró método");
    assert_eq!(method.name, "getX");
    assert!(method.params.is_empty());
    let (obj, field) = as_access(&method.body);
    assert_eq!(as_ident(obj), "self");
    assert_eq!(field, "x");
}

#[test]
fn type_with_inheritance() {
    let p = parse(
        r#"
        type Animal { name = ""; }
        type Dog inherits Animal { bark() => print("woof"); }
        new Dog();
    "#,
    );
    assert_eq!(p.declarations.len(), 2);
    let dog = match &p.declarations[1] {
        Decl::Type(t) => t,
        _ => panic!("segunda declaración no es tipo"),
    };
    assert_eq!(dog.name, "Dog");
    assert_eq!(dog.parent.as_ref().unwrap().name(), "Animal");
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tests — Protocolos
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn protocol_simple() {
    let p = parse("protocol Printable { toString(): String; } 0;");
    let proto = first_protocol(&p);
    assert_eq!(proto.name, "Printable");
    assert!(proto.extends.is_none());
    assert_eq!(proto.methods.len(), 1);
    assert_eq!(proto.methods[0].name, "toString");
    assert_eq!(proto.methods[0].return_type.name(), "String");
}

#[test]
fn protocol_extends() {
    let p = parse("protocol Equatable extends Hashable { equals(other: Object): Boolean; } 0;");
    let proto = first_protocol(&p);
    assert_eq!(proto.extends.as_ref().unwrap().name(), "Hashable");
    assert_eq!(proto.methods[0].params.len(), 1);
    assert_eq!(proto.methods[0].params[0].name, "other");
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tests — Programas completos (end-to-end)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn program_fibonacci() {
    let src = r#"
        function fib(n) =>
            if (n == 0 | n == 1) n
            else fib(n-1) + fib(n-2);

        print(fib(10));
    "#;
    let p = parse(src);
    assert_eq!(p.declarations.len(), 1);
    let (callee, args) = as_call(entry(&p));
    assert_eq!(as_ident(callee), "print");
    let (inner_callee, _) = as_call(&args[0]);
    assert_eq!(as_ident(inner_callee), "fib");
}

#[test]
fn program_gcd() {
    let src = r#"
        function gcd(a, b) => while (a > 0)
            let m = a % b in {
                b := a;
                a := m;
            };

        print(gcd(48, 18));
    "#;
    let p = parse(src);
    let f = first_func(&p);
    assert_eq!(f.name, "gcd");
    assert!(matches!(f.body.kind, ExprKind::While(_)));
}

#[test]
fn program_let_chained() {
    let src = r#"
        let a = 6 in
            let b = a * 7 in
                print(b);
    "#;
    let p = parse(src);
    let (bindings_outer, body_outer) = as_let(entry(&p));
    assert_eq!(bindings_outer[0].name, "a");
    let (bindings_inner, _) = as_let(body_outer);
    assert_eq!(bindings_inner[0].name, "b");
}

#[test]
fn program_type_with_polymorphism() {
    let src = r#"
        type Person(firstname, lastname) {
            firstname = firstname;
            lastname  = lastname;
            name() => firstname @@ lastname;
        }

        type Knight inherits Person {
            name() => "Sir" @@ base();
        }

        let p = new Knight("Phil", "Collins") in print(p.name());
    "#;
    let p = parse(src);
    assert_eq!(p.declarations.len(), 2);
    // Entry: let p = new Knight(...) in print(p.name())
    let (bindings, body) = as_let(entry(&p));
    assert_eq!(bindings[0].name, "p");
    assert!(matches!(bindings[0].value.kind, ExprKind::New(_)));
    let (callee, args) = as_call(body);
    assert_eq!(as_ident(callee), "print");
    let (obj, method, _) = as_method_call(&args[0]);
    assert_eq!(as_ident(obj), "p");
    assert_eq!(method, "name");
}

#[test]
fn program_for_with_range() {
    let src = r#"
        let numbers = [1, 2, 3, 4, 5] in
            for (x in numbers) print(x);
    "#;
    let p = parse(src);
    let (bindings, body) = as_let(entry(&p));
    // binding[0].value es un vector explícito
    match bindings[0].value.as_ref() {
        ExprKind::Vector(v) => match v.as_ref() {
            VectorExpr::Explicit { elements, .. } => assert_eq!(elements.len(), 5),
            _ => panic!("esperaba Explicit"),
        },
        _ => panic!("esperaba Vector"),
    }
    let (var, _, _) = as_for(body);
    assert_eq!(var, "x");
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tests — Errores esperados
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn error_missing_else() {
    // if sin else es error en HULK
    let err = parse_err("if (true) 1;");
    assert!(!err.is_empty(), "debería haber mensaje de error");
}

#[test]
fn error_empty_input() {
    let err = parse_err("");
    assert!(!err.is_empty());
}
