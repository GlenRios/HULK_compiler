// src/parser/tests/real_programs.rs
//
// Tests end-to-end con programas HULK reales completos.
// Cada test verifica que el programa parsea correctamente
// Y que la estructura del AST es la esperada.

use crate::parser::engine::global_driver;
use crate::parser::ast::*;
use crate::lexer::lexer::Lexer;
use crate::lexer::master_nfa::MasterNFA;
use crate::lexer::token_definition::TokenDefinition;

fn tokenize(src: &str) -> Vec<crate::lexer::token::Token> {
    let defs   = TokenDefinition::default_token_definitions();
    let master = MasterNFA::from_token_definitions(&defs);
    let mut lexer = Lexer::new(src, master);
    lexer.tokenize()
}

fn parse(src: &str) -> Program {
    let tokens = tokenize(src);
    global_driver()
        .parse(tokens.into_iter())
        .unwrap_or_else(|e| panic!("Parse falló:\n--- código ---\n{}\n--- error ---\n{}", src, e))
}

// ─────────────────────────────────────────────────────────────────────────────
//  Programa 1 — Fibonacci iterativo
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn real_fibonacci() {
    let src = r#"
        function fib(n) =>
            if (n == 0 | n == 1) n
            else fib(n - 1) + fib(n - 2);

        print(fib(10));
    "#;

    let p = parse(src);

    // Una declaración de función
    assert_eq!(p.declarations.len(), 1);
    let Decl::Function(f) = &p.declarations[0] else { panic!("debe ser FuncDecl") };
    assert_eq!(f.name, "fib");
    assert_eq!(f.params.len(), 1);
    assert_eq!(f.params[0].name, "n");

    // El cuerpo es un if
    assert!(matches!(f.body.as_ref(), Expr::If(_)));

    // La entrada es print(fib(10))
    let Expr::Call(outer) = p.entry.as_ref() else { panic!("entry debe ser Call") };
    let Expr::Identifier { name, .. } = outer.callee.as_ref() else { panic!() };
    assert_eq!(name, "print");
    assert_eq!(outer.args.len(), 1);
    let Expr::Call(inner) = &outer.args[0] else { panic!("arg debe ser Call") };
    let Expr::Identifier { name: fname, .. } = inner.callee.as_ref() else { panic!() };
    assert_eq!(fname, "fib");
}

// ─────────────────────────────────────────────────────────────────────────────
//  Programa 2 — Contador con tipo OOP
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn real_counter_type() {
    let src = r#"
        type Counter(start) {
            count = start;

            increment() => self.count := self.count + 1;
            decrement() => self.count := self.count - 1;
            value()     => self.count;
        }

        let c = new Counter(0) in {
            c.increment();
            c.increment();
            c.increment();
            c.decrement();
            print(c.value());
        };
    "#;

    let p = parse(src);

    assert_eq!(p.declarations.len(), 1);
    let Decl::Type(t) = &p.declarations[0] else { panic!() };
    assert_eq!(t.name, "Counter");
    assert_eq!(t.type_args.len(), 1);
    assert_eq!(t.type_args[0].name, "start");

    // 1 atributo + 3 métodos
    let attrs = t.members.iter().filter(|m| matches!(m, TypeMember::Attribute(_))).count();
    let meths = t.members.iter().filter(|m| matches!(m, TypeMember::Method(_))).count();
    assert_eq!(attrs, 1);
    assert_eq!(meths, 3);

    // La entrada es un let con new Counter(0)
    let Expr::Let(let_e) = p.entry.as_ref() else { panic!() };
    assert_eq!(let_e.bindings[0].name, "c");
    assert!(matches!(let_e.bindings[0].value.as_ref(), Expr::New(_)));

    // El body del let es un bloque con 5 expresiones
    let Expr::Block(block) = let_e.body.as_ref() else { panic!() };
    assert_eq!(block.body.len(), 5);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Programa 3 — Herencia y polimorfismo
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn real_inheritance() {
    let src = r#"
        type Shape {
            area() => 0;
            describe() => "Shape with area " @ area();
        }

        type Circle(r) inherits Shape {
            r = r;
            area() => 3 * r * r;
        }

        type Rectangle(w, h) inherits Shape {
            w = w;
            h = h;
            area() => w * h;
        }

        let c = new Circle(5) in
        let r = new Rectangle(3, 4) in
            print(c.area() + r.area());
    "#;

    let p = parse(src);
    assert_eq!(p.declarations.len(), 3);

    // Verificar Shape
    let Decl::Type(shape) = &p.declarations[0] else { panic!() };
    assert_eq!(shape.name, "Shape");
    assert!(shape.parent.is_none());

    // Verificar Circle hereda Shape
    let Decl::Type(circle) = &p.declarations[1] else { panic!() };
    assert_eq!(circle.name, "Circle");
    assert_eq!(circle.parent.as_ref().unwrap().name(), "Shape");

    // Verificar Rectangle hereda Shape
    let Decl::Type(rect) = &p.declarations[2] else { panic!() };
    assert_eq!(rect.name, "Rectangle");
    assert_eq!(rect.parent.as_ref().unwrap().name(), "Shape");

    // La entrada es un let anidado
    let Expr::Let(outer_let) = p.entry.as_ref() else { panic!() };
    assert_eq!(outer_let.bindings[0].name, "c");
    let Expr::Let(inner_let) = outer_let.body.as_ref() else { panic!() };
    assert_eq!(inner_let.bindings[0].name, "r");
}

// ─────────────────────────────────────────────────────────────────────────────
//  Programa 4 — Funciones de orden superior y let complejo
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn real_higher_order() {
    let src = r#"
        function apply(f, x) => f(x);
        function double(x)   => x * 2;
        function square(x)   => x * x;

        let result = apply(double, apply(square, 3)) in
            print(result);
    "#;

    let p = parse(src);
    assert_eq!(p.declarations.len(), 3);

    let names: Vec<&str> = p.declarations.iter().map(|d| d.name()).collect();
    assert_eq!(names, vec!["apply", "double", "square"]);

    // entry: let result = apply(...) in print(result)
    let Expr::Let(let_e) = p.entry.as_ref() else { panic!() };
    assert_eq!(let_e.bindings[0].name, "result");

    // El valor del binding es apply(double, apply(square, 3))
    let Expr::Call(outer_call) = let_e.bindings[0].value.as_ref() else { panic!() };
    let Expr::Identifier { name, .. } = outer_call.callee.as_ref() else { panic!() };
    assert_eq!(name, "apply");
    assert_eq!(outer_call.args.len(), 2);

    // El segundo arg es apply(square, 3)
    let Expr::Call(inner_call) = &outer_call.args[1] else { panic!() };
    let Expr::Identifier { name: iname, .. } = inner_call.callee.as_ref() else { panic!() };
    assert_eq!(iname, "apply");
}

// ─────────────────────────────────────────────────────────────────────────────
//  Programa 5 — While con destructive assignment
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn real_while_loop() {
    let src = r#"
        function sum_to(n) =>
            let total = 0 in
            let i = 0 in {
                while (i <= n) {
                    total := total + i;
                    i := i + 1;
                };
                total
            };

        print(sum_to(100));
    "#;

    let p = parse(src);
    assert_eq!(p.declarations.len(), 1);

    let Decl::Function(f) = &p.declarations[0] else { panic!() };
    assert_eq!(f.name, "sum_to");

    // body es let total = 0 in let i = 0 in { while ... }
    let Expr::Let(outer) = f.body.as_ref() else { panic!("body debe ser Let") };
    assert_eq!(outer.bindings[0].name, "total");
    let Expr::Let(inner) = outer.body.as_ref() else { panic!("inner debe ser Let") };
    assert_eq!(inner.bindings[0].name, "i");
    let Expr::Block(block) = inner.body.as_ref() else { panic!("debe ser Block") };

    // Primer elemento del bloque es while
    assert!(matches!(block.body[0], Expr::While(_)));
    // Último elemento (valor del bloque) es total
    assert!(matches!(block.tail(), Expr::Identifier { .. }));
}

// ─────────────────────────────────────────────────────────────────────────────
//  Programa 6 — Protocolo e implementación implícita
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn real_protocol() {
    let src = r#"
        protocol Printable {
            to_string(): String;
        }

        type Point(x, y) {
            x = x;
            y = y;
            to_string(): String => "(" @ x @ ", " @ y @ ")";
        }

        let p = new Point(3, 4) in print(p.to_string());
    "#;

    let p = parse(src);
    assert_eq!(p.declarations.len(), 2);

    let Decl::Protocol(proto) = &p.declarations[0] else { panic!() };
    assert_eq!(proto.name, "Printable");
    assert_eq!(proto.methods.len(), 1);
    assert_eq!(proto.methods[0].name, "to_string");
    assert_eq!(proto.methods[0].return_type.name(), "String");

    let Decl::Type(point) = &p.declarations[1] else { panic!() };
    assert_eq!(point.name, "Point");

    // entry: let p = new Point(3,4) in print(p.to_string())
    let Expr::Let(let_e) = p.entry.as_ref() else { panic!() };
    let Expr::New(new_e) = let_e.bindings[0].value.as_ref() else { panic!() };
    assert_eq!(new_e.type_name.name(), "Point");
    assert_eq!(new_e.args.len(), 2);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Programa 7 — For loop con vector
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn real_for_and_vector() {
    let src = r#"
        function sum(numbers, n) =>
            let total = 0 in {
                for (x in numbers) total := total + x;
                total
            };

        let v = [1, 2, 3, 4, 5] in print(sum(v, 5));
    "#;

    let p = parse(src);
    assert_eq!(p.declarations.len(), 1);

    // La entrada es let v = [...] in print(sum(v, 5))
    let Expr::Let(let_e) = p.entry.as_ref() else { panic!() };
    assert_eq!(let_e.bindings[0].name, "v");

    // El valor del binding es un vector explícito de 5 elementos
    let Expr::Vector(vec_e) = let_e.bindings[0].value.as_ref() else { panic!() };
    let VectorExpr::Explicit { elements, .. } = vec_e.as_ref() else { panic!() };
    assert_eq!(elements.len(), 5);

    // El body llama a print(sum(v, 5))
    let Expr::Call(print_call) = let_e.body.as_ref() else { panic!() };
    let Expr::Identifier { name, .. } = print_call.callee.as_ref() else { panic!() };
    assert_eq!(name, "print");
}

// ─────────────────────────────────────────────────────────────────────────────
//  Programa 8 — if/elif/else como expresión dentro de función
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn real_if_as_expression() {
    let src = r#"
        function classify(n) =>
            if (n < 0)       "negative"
            elif (n == 0)    "zero"
            elif (n < 10)    "small"
            elif (n < 100)   "medium"
            else             "large";

        print(classify(42));
    "#;

    let p = parse(src);

    let Decl::Function(f) = &p.declarations[0] else { panic!() };
    assert_eq!(f.name, "classify");

    // El cuerpo es un if con 3 elifs
    let Expr::If(if_e) = f.body.as_ref() else { panic!("debe ser If") };
    assert_eq!(if_e.elif_chain.len(), 3);

    // Verificar las condiciones de cada rama
    // then: n < 0
    let Expr::Binary(b) = if_e.condition.as_ref() else { panic!() };
    assert_eq!(b.op, BinaryOp::Less);

    // elif[0]: n == 0
    let Expr::Binary(b2) = if_e.elif_chain[0].condition.as_ref() else { panic!() };
    assert_eq!(b2.op, BinaryOp::Eq);

    // else: "large"
    assert!(matches!(if_e.else_body.as_ref(), Expr::Literal(Literal::String { .. })));
}

// ─────────────────────────────────────────────────────────────────────────────
//  Programa 9 — Múltiples funciones que se llaman entre sí
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn real_mutual_functions() {
    let src = r#"
        function is_even(n) => if (n == 0) true  else is_odd(n - 1);
        function is_odd(n)  => if (n == 0) false else is_even(n - 1);

        print(is_even(10));
    "#;

    let p = parse(src);
    assert_eq!(p.declarations.len(), 2);
    assert_eq!(p.declarations[0].name(), "is_even");
    assert_eq!(p.declarations[1].name(), "is_odd");

    // Ambas funciones tienen if como cuerpo
    for decl in &p.declarations {
        let Decl::Function(f) = decl else { panic!() };
        assert!(matches!(f.body.as_ref(), Expr::If(_)));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Programa 10 — Programa completo de banco
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn real_bank_account() {
    let src = r#"
        type BankAccount(owner, initial) {
            owner   = owner;
            balance = initial;

            deposit(amount)  => self.balance := self.balance + amount;
            withdraw(amount) => if (amount <= self.balance)
                                    self.balance := self.balance - amount
                                else
                                    print("Insufficient funds");
            get_balance()    => self.balance;
            get_owner()      => self.owner;
        }

        let account = new BankAccount("Alice", 1000) in {
            account.deposit(500);
            account.withdraw(200);
            print(account.get_balance());
        };
    "#;

    let p = parse(src);
    assert_eq!(p.declarations.len(), 1);

    let Decl::Type(t) = &p.declarations[0] else { panic!() };
    assert_eq!(t.name, "BankAccount");
    assert_eq!(t.type_args.len(), 2);

    let methods: Vec<&str> = t.members.iter().filter_map(|m| {
        if let TypeMember::Method(meth) = m { Some(meth.name.as_str()) } else { None }
    }).collect();
    assert!(methods.contains(&"deposit"));
    assert!(methods.contains(&"withdraw"));
    assert!(methods.contains(&"get_balance"));
    assert!(methods.contains(&"get_owner"));

    // entry: let account = new BankAccount("Alice", 1000) in { ... }
    let Expr::Let(let_e) = p.entry.as_ref() else { panic!() };
    let Expr::New(new_e) = let_e.bindings[0].value.as_ref() else { panic!() };
    assert_eq!(new_e.type_name.name(), "BankAccount");
    assert_eq!(new_e.args.len(), 2);

    let Expr::Block(block) = let_e.body.as_ref() else { panic!() };
    assert_eq!(block.body.len(), 3);

    // Todas las expresiones del bloque son method calls
    for expr in &block.body {
        assert!(matches!(expr, Expr::MethodCall(_) | Expr::Call(_)));
    }
}