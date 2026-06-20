# Módulo `parser/ast/`

Documentación completa del módulo de nodos del Árbol de Sintaxis Abstracta (AST)
del compilador HULK.

---

## ¿Qué es el AST y para qué sirve?

El **AST (Abstract Syntax Tree)** es la representación del programa en memoria
después de que el parser terminó su trabajo. Es una estructura de árbol donde:

- Cada **nodo** representa una construcción del lenguaje (`let`, `if`, `+`, una llamada a función, etc.)
- Los **hijos** de un nodo son las subexpresiones que lo componen
- Las **hojas** son los valores atómicos: literales y nombres de variables

Ejemplo: el programa `let x = 1 + 2 in print(x)` genera este árbol:

```
LetExpr
├── binding: x = BinaryExpr(Add, 1, 2)
│                ├── left:  Literal(Number("1"))
│                └── right: Literal(Number("2"))
└── body: CallExpr
          ├── callee: Identifier("print")
          └── args:   [Identifier("x")]
```

---

## Estructura de archivos

```
parser/ast/
├── mod.rs               ← punto de entrada, re-exports, tests de integración
├── span.rs              ← posición en el código fuente (línea:columna)
├── types.rs             ← TypeName (nombre de tipo en el código)
├── program.rs           ← Program (nodo raíz del árbol)
├── decl/
│   ├── mod.rs           ← enum Decl (unión de las tres declaraciones)
│   ├── func_decl.rs     ← FuncDecl, Param
│   ├── type_decl.rs     ← TypeDecl, AttributeDef, MethodDef, TypeMember
│   └── protocol_decl.rs ← ProtocolDecl, MethodSignature
└── expr/
    ├── mod.rs           ← enum Expr (el nodo central de todo)
    ├── literal.rs       ← Literal (Number, String, Char, Bool, Null)
    ├── binary.rs        ← BinaryExpr, BinaryOp
    ├── unary.rs         ← UnaryExpr, UnaryOp, PostfixExpr, PostfixOp
    ├── assign.rs        ← AssignExpr, AssignOp
    ├── block.rs         ← BlockExpr
    ├── let_expr.rs      ← LetExpr, LetBinding
    ├── if_expr.rs       ← IfExpr, ElifBranch
    ├── while_expr.rs    ← WhileExpr
    ├── for_expr.rs      ← ForExpr
    ├── new_expr.rs      ← NewExpr
    ├── call_access.rs   ← CallExpr, AccessExpr, MethodCallExpr, IndexExpr
    └── vector.rs        ← VectorExpr
```

---

## `span.rs` — Posición en el código fuente

```rust
pub struct Span {
    pub line:   usize,
    pub column: usize,
}
```

`Span` es un struct pequeño que todos los nodos del AST cargan consigo.
Sirve para que cuando el analizador semántico encuentre un error, pueda
decirle al usuario **exactamente dónde** está el problema:

```
Error [3:12]: variable 'x' no está definida en este scope
```

### Constructores

```rust
Span::new(3, 12)   // línea 3, columna 12
Span::dummy()      // posición vacía — solo para tests
```

> **Diseño importante**: `Span` es `Copy` (se copia automáticamente como un entero),
> por lo que no hay overhead de clonar posiciones a lo largo del árbol.

---

## `types.rs` — Nombres de tipo

`TypeName` representa cómo el programador escribe un tipo en el código.
HULK tiene tres formas sintácticas:

```rust
pub enum TypeName {
    Simple   { name: String, span: Span },  // Number, Point, Object
    Vector   { name: String, span: Span },  // Number[]
    Iterable { name: String, span: Span },  // Number*
}
```

### Constructores

```rust
TypeName::simple("Number", span)      // →  Number
TypeName::vector("Number", span)      // →  Number[]
TypeName::iterable("Number", span)    // →  Number*
```

### Métodos

```rust
type_name.name()   // → &str  — el nombre base ("Number" en todos los casos)
type_name.span()   // → Span
```

### Display

```rust
format!("{}", TypeName::simple("Number", s))      // "Number"
format!("{}", TypeName::vector("Number", s))      // "Number[]"
format!("{}", TypeName::iterable("Number", s))    // "Number*"
```

> `Number*` solo es válido en anotaciones de parámetros de función.
> El AST lo acepta en cualquier posición; el analizador semántico
> restringe su uso.

---

## `program.rs` — El nodo raíz

```rust
pub struct Program {
    pub declarations: Vec<Decl>,   // funciones, tipos, protocolos
    pub entry: Box<Expr>,          // la expresión global de entrada
    pub span:  Span,
}
```

El nodo raíz del árbol. Corresponde directamente a la producción:
```
Program → DeclList Expr
```

Ejemplo para el programa:
```js
function double(x) => x * 2;
print(double(21));
```

El AST sería:
```
Program {
    declarations: [
        Decl::Function(FuncDecl { name: "double", ... })
    ],
    entry: CallExpr {
        callee: Identifier("print"),
        args: [CallExpr { callee: Identifier("double"), args: [Literal(Number("21"))] }]
    }
}
```

---

## `decl/` — Declaraciones de nivel superior

### `enum Decl` (`decl/mod.rs`)

La unión de las tres posibles declaraciones:

```rust
pub enum Decl {
    Function(FuncDecl),
    Type(TypeDecl),
    Protocol(ProtocolDecl),
}
```

Métodos:
```rust
decl.span()   // → Span
decl.name()   // → &str — el nombre de la función/tipo/protocolo
```

---

### `FuncDecl` y `Param` (`decl/func_decl.rs`)

#### `Param` — Un parámetro de función

```rust
pub struct Param {
    pub name:     String,
    pub type_ann: Option<TypeName>,   // None si no hay anotación
    pub span:     Span,
}
```

Cubre ambas formas:
```js
function f(x)          // Param { name: "x", type_ann: None }
function f(x: Number)  // Param { name: "x", type_ann: Some(Simple("Number")) }
```

#### `FuncDecl` — Declaración de función

```rust
pub struct FuncDecl {
    pub name:        String,
    pub params:      Vec<Param>,
    pub return_type: Option<TypeName>,  // None si no hay anotación
    pub body:        Box<Expr>,         // inline: cualquier Expr; completa: Expr::Block
    pub span:        Span,
}
```

Cubre las cuatro formas sintácticas:

```js
// inline sin tipo
function tan(x) => sin(x) / cos(x);
// FuncDecl { name: "tan", params: [x], return_type: None, body: BinaryExpr(Div,...) }

// completa sin tipo
function operate(x, y) { print(x+y); print(x-y); }
// FuncDecl { name: "operate", body: BlockExpr([...]) }

// inline con tipo
function tan(x: Number): Number => sin(x) / cos(x);
// FuncDecl { return_type: Some(Simple("Number")), ... }

// completa con tipo
function f(x: Number): Number { ... }
```

> El campo `body` es siempre un `Box<Expr>`. Para funciones inline es
> directamente la expresión; para funciones completas es un `Expr::Block`.
> No hay distinción en el tipo — el analizador semántico los trata igual.

---

### `TypeDecl`, `AttributeDef`, `MethodDef` (`decl/type_decl.rs`)

#### `AttributeDef` — Atributo de un tipo

```rust
pub struct AttributeDef {
    pub name:     String,
    pub type_ann: Option<TypeName>,
    pub value:    Box<Expr>,
    pub span:     Span,
}
```

```js
x = 0;             // AttributeDef { name: "x", type_ann: None, value: Literal(0) }
x: Number = 0;     // AttributeDef { name: "x", type_ann: Some("Number"), value: Literal(0) }
```

#### `MethodDef` — Método de un tipo

```rust
pub struct MethodDef {
    pub name:        String,
    pub params:      Vec<Param>,
    pub return_type: Option<TypeName>,
    pub body:        Box<Expr>,
    pub span:        Span,
}
```

Idéntico a `FuncDecl` pero sin la palabra clave `function`.

#### `TypeMember` — Unión de atributo o método

```rust
pub enum TypeMember {
    Attribute(AttributeDef),
    Method(MethodDef),
}
```

#### `TypeDecl` — Declaración de tipo

```rust
pub struct TypeDecl {
    pub name:        String,
    pub type_args:   Vec<Param>,     // args del constructor: type Point(x, y)
    pub parent:      Option<TypeName>,   // None si no hereda explícitamente
    pub parent_args: Vec<Expr>,      // args al padre: inherits Point(rho*sin(phi))
    pub members:     Vec<TypeMember>,
    pub span:        Span,
}
```

Ejemplos:

```js
// Sin args ni herencia
type Counter {
    count = 0;
    increment() => self.count := self.count + 1;
}
// TypeDecl { name: "Counter", type_args: [], parent: None, parent_args: [], ... }

// Con args y herencia con args
type PolarPoint(phi, rho) inherits Point(rho * sin(phi), rho * cos(phi)) {
    rho() => sqrt(self.getX()^2 + self.getY()^2);
}
// TypeDecl {
//   name: "PolarPoint",
//   type_args: [phi, rho],
//   parent: Some(Simple("Point")),
//   parent_args: [BinaryExpr(Mul, rho, Call(sin, phi)), ...],
//   members: [MethodDef { name: "rho", ... }]
// }
```

---

### `ProtocolDecl` y `MethodSignature` (`decl/protocol_decl.rs`)

#### `MethodSignature` — Firma de método en protocolo

```rust
pub struct MethodSignature {
    pub name:        String,
    pub params:      Vec<Param>,
    pub return_type: TypeName,    // OBLIGATORIO en protocolos (no Option)
    pub span:        Span,
}
```

```js
hash(): Number;                    // MethodSignature { name: "hash", params: [], return_type: Number }
equals(other: Object): Boolean;    // MethodSignature { name: "equals", params: [other:Object], ... }
```

#### `ProtocolDecl` — Declaración de protocolo

```rust
pub struct ProtocolDecl {
    pub name:    String,
    pub extends: Option<TypeName>,   // None si no extiende ningún protocolo
    pub methods: Vec<MethodSignature>,
    pub span:    Span,
}
```

```js
protocol Hashable {
    hash(): Number;
}
// ProtocolDecl { name: "Hashable", extends: None, methods: [MethodSignature { name: "hash" }] }

protocol Equatable extends Hashable {
    equals(other: Object): Boolean;
}
// ProtocolDecl { name: "Equatable", extends: Some(Simple("Hashable")), ... }
```

---

## `expr/` — Expresiones

### El enum central: `Expr` (`expr/mod.rs`)

`Expr` es el nodo más importante de todo el AST. Todas las expresiones
del lenguaje son variantes de este enum:

```rust
pub enum Expr {
    // Átomos
    Literal(Literal),
    Identifier { name: String, span: Span },
    Base(Span),

    // Operaciones
    Binary(Box<BinaryExpr>),
    Unary(Box<UnaryExpr>),
    Postfix(Box<PostfixExpr>),
    Assign(Box<AssignExpr>),

    // Comprobación/conversión de tipos
    Is { expr: Box<Expr>, type_name: TypeName, span: Span },
    As { expr: Box<Expr>, type_name: TypeName, span: Span },

    // Llamadas y accesos
    Call(Box<CallExpr>),
    Access(Box<AccessExpr>),
    MethodCall(Box<MethodCallExpr>),
    Index(Box<IndexExpr>),

    // Expresiones compuestas
    Block(Box<BlockExpr>),
    Let(Box<LetExpr>),
    If(Box<IfExpr>),
    While(Box<WhileExpr>),
    For(Box<ForExpr>),
    New(Box<NewExpr>),
    Vector(Box<VectorExpr>),
}
```

#### ¿Por qué todos los campos complejos son `Box<...>`?

Porque `Expr` es recursivo — contiene otros `Expr`. Sin `Box`, Rust no
puede calcular el tamaño del tipo en tiempo de compilación (tamaño infinito).
Con `Box`, cada nodo solo ocupa el tamaño de un puntero en el stack,
y el contenido real vive en el heap.

#### Constructores de conveniencia

En vez de escribir:
```rust
Expr::Binary(Box::new(BinaryExpr::new(BinaryOp::Add, left, right, span)))
```

puedes escribir:
```rust
Expr::binary(BinaryOp::Add, left, right, span)
```

Todos los nodos tienen constructores así en `expr/mod.rs`:

```rust
Expr::number("42", span)
Expr::string("hello", span)
Expr::bool(true, span)
Expr::null(span)
Expr::identifier("x", span)
Expr::binary(op, left, right, span)
Expr::unary(op, operand, span)
Expr::assign(op, target, value, span)
Expr::block(body_vec, span)
Expr::let_expr(bindings, body, span)
Expr::if_expr(cond, then, elifs, else_, span)
Expr::while_expr(cond, body, span)
Expr::for_expr("x", iterable, body, span)
Expr::call(callee, args, span)
Expr::method_call(object, "method", args, span)
Expr::access(object, "field", span)
Expr::index(collection, index, span)
```

#### Método `span()`

```rust
expr.span()  // → Span
```

Disponible en todas las variantes. Permite al analizador semántico obtener
la posición de cualquier nodo sin necesidad de hacer pattern matching manualmente.

---

### `Literal` (`expr/literal.rs`)

```rust
pub enum Literal {
    Number { value: String, span: Span },  // "42", "3.14"
    String { value: String, span: Span },  // "hello world"
    Char   { value: String, span: Span },  // "a"
    Bool   { value: bool,   span: Span },  // true / false
    Null   { span: Span },
}
```

> **Decisión de diseño clave**: los números se guardan como `String`, no como `f64`.
> El parser no convierte `"42"` a un número — eso es responsabilidad del
> analizador semántico o la generación de código. Esto evita perder precisión
> y permite reportar errores de conversión con contexto.

---

### `BinaryExpr` y `BinaryOp` (`expr/binary.rs`)

Representa operaciones con dos operandos: `left op right`.

```rust
pub enum BinaryOp {
    Add, Sub, Mul, Div, Mod,
    Power,           // ^ o **
    Eq, NotEq,
    Less, Greater, LessEq, GreaterEq,
    And,             // &
    Or,              // |
    Concat,          // @
    DoubleConcat,    // @@
}

pub struct BinaryExpr {
    pub op:    BinaryOp,
    pub left:  Box<Expr>,
    pub right: Box<Expr>,
    pub span:  Span,
}
```

Ejemplos:
```js
1 + 2           // BinaryExpr { op: Add, left: Literal(1), right: Literal(2) }
"a" @ "b"       // BinaryExpr { op: Concat, left: Literal("a"), right: Literal("b") }
x == y          // BinaryExpr { op: Eq, left: Identifier("x"), right: Identifier("y") }
```

---

### `UnaryExpr`, `PostfixExpr` (`expr/unary.rs`)

```rust
pub enum UnaryOp {
    Neg,  // -x
    Not,  // !x
}

pub struct UnaryExpr {
    pub op:      UnaryOp,
    pub operand: Box<Expr>,
    pub span:    Span,
}

pub enum PostfixOp {
    Increment,  // x++
    Decrement,  // x--
}

pub struct PostfixExpr {
    pub op:      PostfixOp,
    pub operand: Box<Expr>,
    pub span:    Span,
}
```

---

### `AssignExpr` y `AssignOp` (`expr/assign.rs`)

```rust
pub enum AssignOp {
    Assign,      // :=
    PlusAssign,  // +=
    MinusAssign, // -=
    MulAssign,   // *=
    DivAssign,   // /=
    ModAssign,   // %=
}

pub struct AssignExpr {
    pub op:     AssignOp,
    pub target: Box<Expr>,  // lvalue — validado por el semántico
    pub value:  Box<Expr>,
    pub span:   Span,
}
```

```js
x := 42        // AssignExpr { op: Assign, target: Identifier("x"), value: Literal(42) }
self.x := y    // AssignExpr { op: Assign, target: Access(Base, "x"), value: Identifier("y") }
```

> El `target` es un `Expr` genérico en el AST. La restricción de que solo
> puede ser una variable o atributo la aplica el analizador semántico.

---

### `BlockExpr` (`expr/block.rs`)

```rust
pub struct BlockExpr {
    pub body: Vec<Expr>,   // nunca vacío
    pub span: Span,
}
```

```js
{
    print(1);
    print(2);
    42
}
// BlockExpr { body: [Call("print",1), Call("print",2), Literal(42)] }
// El valor del bloque es la última expresión: Literal(42)
```

Método especial:
```rust
block.tail()  // → &Expr — la última expresión (el valor del bloque)
```

---

### `LetExpr` y `LetBinding` (`expr/let_expr.rs`)

```rust
pub struct LetBinding {
    pub name:     String,
    pub type_ann: Option<TypeName>,
    pub value:    Box<Expr>,
    pub span:     Span,
}

pub struct LetExpr {
    pub bindings: Vec<LetBinding>,
    pub body:     Box<Expr>,
    pub span:     Span,
}
```

```js
let x = 42 in print(x)
// LetExpr {
//   bindings: [LetBinding { name: "x", type_ann: None, value: Literal(42) }],
//   body: Call("print", [Identifier("x")])
// }

let a = 6, b: Number = a * 7 in print(b)
// LetExpr {
//   bindings: [
//     LetBinding { name: "a", type_ann: None, value: Literal(6) },
//     LetBinding { name: "b", type_ann: Some("Number"), value: Binary(Mul, a, 7) }
//   ],
//   body: Call("print", [Identifier("b")])
// }
```

> Los múltiples bindings son semánticamente equivalentes a `let` anidados,
> pero el AST los mantiene planos para facilitar el recorrido.

---

### `IfExpr` y `ElifBranch` (`expr/if_expr.rs`)

```rust
pub struct ElifBranch {
    pub condition: Box<Expr>,
    pub body:      Box<Expr>,
    pub span:      Span,
}

pub struct IfExpr {
    pub condition:  Box<Expr>,
    pub then_body:  Box<Expr>,
    pub elif_chain: Vec<ElifBranch>,  // puede estar vacío
    pub else_body:  Box<Expr>,        // SIEMPRE presente en HULK
    pub span:       Span,
}
```

```js
if (x > 0) "positive"
elif (x < 0) "negative"
else "zero"

// IfExpr {
//   condition: Binary(Greater, x, 0),
//   then_body: Literal("positive"),
//   elif_chain: [
//     ElifBranch { condition: Binary(Less, x, 0), body: Literal("negative") }
//   ],
//   else_body: Literal("zero")
// }
```

> El campo `else_body` es `Box<Expr>`, no `Option<Box<Expr>>`,
> porque en HULK el `else` es **obligatorio**. No existe el problema
> del *dangling else*.

---

### `WhileExpr` (`expr/while_expr.rs`)

```rust
pub struct WhileExpr {
    pub condition: Box<Expr>,
    pub body:      Box<Expr>,
    pub span:      Span,
}
```

```js
while (x >= 0) { print(x); x := x - 1; }
// WhileExpr {
//   condition: Binary(GreaterEq, x, 0),
//   body: Block([Call("print", [x]), Assign(x, Binary(Sub, x, 1))])
// }
```

---

### `ForExpr` (`expr/for_expr.rs`)

```rust
pub struct ForExpr {
    pub var:      String,      // nombre de la variable de iteración
    pub iterable: Box<Expr>,
    pub body:     Box<Expr>,
    pub span:     Span,
}
```

```js
for (x in range(0, 10)) print(x)
// ForExpr {
//   var: "x",
//   iterable: Call("range", [Literal(0), Literal(10)]),
//   body: Call("print", [Identifier("x")])
// }
```

> El `for` se puede transpiliar a `while` en fases posteriores del compilador,
> pero el AST lo mantiene como nodo propio para claridad.

---

### `NewExpr` (`expr/new_expr.rs`)

```rust
pub struct NewExpr {
    pub type_name: TypeName,
    pub args:      Vec<Expr>,
    pub span:      Span,
}
```

```js
new Point(3, 4)
// NewExpr {
//   type_name: Simple("Point"),
//   args: [Literal(3), Literal(4)]
// }
```

---

### `CallExpr`, `AccessExpr`, `MethodCallExpr`, `IndexExpr` (`expr/call_access.rs`)

Cuatro nodos distintos para los cuatro tipos de acceso/llamada:

```rust
// f(args)  —  llamada a función o functor
pub struct CallExpr {
    pub callee: Box<Expr>,
    pub args:   Vec<Expr>,
    pub span:   Span,
}

// obj.field  —  acceso a atributo
pub struct AccessExpr {
    pub object: Box<Expr>,
    pub field:  String,
    pub span:   Span,
}

// obj.method(args)  —  llamada a método
pub struct MethodCallExpr {
    pub object: Box<Expr>,
    pub method: String,
    pub args:   Vec<Expr>,
    pub span:   Span,
}

// collection[index]  —  indexación
pub struct IndexExpr {
    pub collection: Box<Expr>,
    pub index:      Box<Expr>,
    pub span:       Span,
}
```

> `MethodCallExpr` existe como nodo separado de `CallExpr + AccessExpr`
> porque el analizador semántico necesita resolver el método en el tipo
> del objeto, que es distinto de resolver una función global.

Ejemplo de encadenamiento:

```js
obj.method(1)(2)[0].field
// Esto se representa como árbol anidado:
// Access(
//   Index(
//     Call(
//       MethodCall(obj, "method", [1]),
//       [2]
//     ),
//     0
//   ),
//   "field"
// )
```

---

### `VectorExpr` (`expr/vector.rs`)

```rust
pub enum VectorExpr {
    Explicit {
        elements: Vec<Expr>,
        span:     Span,
    },
    Generator {
        body:     Box<Expr>,   // expresión a evaluar para cada elemento
        var:      String,
        iterable: Box<Expr>,
        span:     Span,
    },
}
```

```js
// Forma explícita
[1, 2, 3]
// VectorExpr::Explicit { elements: [Literal(1), Literal(2), Literal(3)] }

// Vector vacío
[]
// VectorExpr::Explicit { elements: [] }

// Forma generadora
[x^2 | x in range(1, 10)]
// VectorExpr::Generator {
//   body: Binary(Power, Identifier("x"), Literal(2)),
//   var: "x",
//   iterable: Call("range", [Literal(1), Literal(10)])
// }
```

---

## `mod.rs` — El punto de entrada del módulo

`mod.rs` hace dos cosas:

**1. Re-exporta todos los tipos** para que el resto del compilador pueda usar
rutas cortas en lugar de rutas completas:

```rust
// Sin re-exports (ruta larga — incómodo):
use crate::parser::ast::expr::let_expr::LetBinding;

// Con re-exports (ruta corta):
use crate::parser::ast::LetBinding;
```

**2. Contiene los tests de integración** que verifican que se pueden
construir los nodos más importantes correctamente.

---

## Cómo recorrer el AST

El patrón estándar para recorrer el árbol es el **pattern matching** de Rust.
El analizador semántico, el generador de código, y cualquier otra fase
del compilador lo hace así:

```rust
fn process_expr(expr: &Expr) {
    match expr {
        Expr::Literal(lit) => {
            match lit {
                Literal::Number { value, .. } => { /* convertir a f64 */ }
                Literal::String { value, .. } => { /* internar string */ }
                _ => {}
            }
        }
        Expr::Binary(bin) => {
            process_expr(&bin.left);   // recursión en el hijo izquierdo
            process_expr(&bin.right);  // recursión en el hijo derecho
            // combinar los resultados según bin.op
        }
        Expr::Let(let_e) => {
            for binding in &let_e.bindings {
                process_expr(&binding.value);  // procesar el valor
                // registrar binding.name en la tabla de símbolos
            }
            process_expr(&let_e.body);
        }
        Expr::If(if_e) => {
            process_expr(&if_e.condition);
            process_expr(&if_e.then_body);
            for elif in &if_e.elif_chain {
                process_expr(&elif.condition);
                process_expr(&elif.body);
            }
            process_expr(&if_e.else_body);
        }
        // ... resto de variantes
    }
}
```

---

## Tests disponibles

Los tests de integración están en `ast/mod.rs` y se corren con:

```bash
cargo test ast
cargo test ast -- --nocapture   # ver los println! si los hubiera
```

Tests incluidos:

| Test | Qué verifica |
|------|-------------|
| `build_simple_program` | `Program` con entry `print(42)` |
| `build_binary_expr` | `1 + 2` |
| `build_let_expr` | `let x = 42 in x` con un binding |
| `build_if_expr` | `if (true) 1 else 2` sin elif |
| `build_block_expr` | `{ print(1); print(2) }` y `.tail()` |
| `build_type_decl` | `type Point(x,y)` con un atributo |
| `build_func_decl` | `function id(x) => x` |
| `build_protocol_decl` | `protocol Hashable { hash(): Number; }` |
| `build_vector_explicit` | `[1, 2, 3]` |
| `build_vector_generator` | `[x^2 \| x in range(0,10)]` |
| `type_name_display` | Display de las tres formas de TypeName |

---

## Resumen visual del árbol de tipos

```
Program
├── declarations: Vec<Decl>
│   ├── Decl::Function(FuncDecl)
│   │   ├── name: String
│   │   ├── params: Vec<Param>
│   │   ├── return_type: Option<TypeName>
│   │   └── body: Box<Expr>
│   ├── Decl::Type(TypeDecl)
│   │   ├── name: String
│   │   ├── type_args: Vec<Param>
│   │   ├── parent: Option<TypeName>
│   │   ├── parent_args: Vec<Expr>
│   │   └── members: Vec<TypeMember>
│   │       ├── TypeMember::Attribute(AttributeDef)
│   │       └── TypeMember::Method(MethodDef)
│   └── Decl::Protocol(ProtocolDecl)
│       ├── name: String
│       ├── extends: Option<TypeName>
│       └── methods: Vec<MethodSignature>
└── entry: Box<Expr>
    └── (cualquier variante de Expr)
```