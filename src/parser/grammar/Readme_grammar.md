# Módulo `parser/grammar/`

Documentación completa del módulo de gramática del compilador HULK.

---

## ¿Qué hace este módulo?

Este módulo representa la **gramática formal del lenguaje HULK en código Rust**.
Es el primer eslabón del pipeline del parser: antes de construir tablas LALR,
antes de hacer shift/reduce, antes de construir el AST — lo primero es tener
un modelo en memoria de las reglas del lenguaje.

Piénsalo como una base de datos de las reglas:

```
"AddExpr puede ser: MulExpr, o AddExpr + MulExpr, o AddExpr - MulExpr"
```

Todo lo demás (FIRST, FOLLOW, AFD, tabla ACTION/GOTO) se **deriva** de estas reglas.

---

## Estructura de archivos

```
parser/grammar/
├── mod.rs               ← struct Grammar (el contenedor central)
├── symbol.rs            ← Terminal, NonTerminal, Symbol
├── production.rs        ← struct Production + macros
├── token_translator.rs  ← puente Lexer → Parser
└── hulk_grammar.rs      ← las ~90 producciones reales de HULK
```

---

## `symbol.rs` — Los símbolos de la gramática

### ¿Qué es un símbolo?

En teoría de lenguajes formales, una gramática opera sobre **símbolos**.
Hay dos tipos:

- **Terminales**: los tokens que produce el lexer. Son las "hojas" de la gramática — no se pueden expandir más. Ejemplos: `+`, `if`, `42`, `"hello"`.
- **No-terminales**: conceptos abstractos del lenguaje que se **definen** mediante producciones. Ejemplos: `Expr`, `FuncDecl`, `AddExpr`.

### `enum Terminal`

Cada variante corresponde exactamente a un `TokenType` del lexer.

```rust
pub enum Terminal {
    // Literales
    Number,     // 42, 3.14
    String,     // "hello"
    Char,       // 'a'
    True,       // true
    False,      // false
    Null,       // null

    // Keywords
    If, Elif, Else, While, For,
    Function, Let, In, Type, New,
    Inherits, Base, Protocol, Extends,

    // Operadores de palabra
    Is, As,

    // Operadores multicarácter
    PowerStar,       // **
    Increment,       // ++
    Decrement,       // --
    DoubleConcat,    // @@
    DestructAssign,  // :=
    PlusAssign,      // +=
    MinusAssign,     // -=
    MultAssign,      // *=
    DivAssign,       // /=
    ModAssign,       // %=
    Equal,           // ==
    NotEqual,        // !=
    LessEq,          // <=
    GreaterEq,       // >=
    Arrow,           // =>
    RtArrow,         // ->

    // Operadores de un carácter
    Assign, Less, Greater, Plus, Minus,
    Multiply, Divide, Modulo, PowerCaret,
    And, Or, Not, Concat,

    // Delimitadores
    Semicolon, Colon, Comma, Dot,
    LParen, RParen, LBrace, RBrace, LBracket, RBracket,

    // Identificador y fin de entrada
    Identifier,
    Eof,         // representa el $ de la tabla LALR
}
```

> **¿Por qué duplicamos `TokenType`?**
> Para no acoplar el módulo `parser` al módulo `lexer`.
> Si el lexer cambia internamente, el parser no se entera.
> El puente entre ambos vive en `token_translator.rs`.

### `enum NonTerminal`

Los no-terminales están agrupados por área semántica:

```rust
pub enum NonTerminal {
    // Raíz del programa
    Program, Start,

    // Declaraciones de nivel superior
    DeclList, Decl,
    FuncDecl, TypeDecl, ProtocolDecl,

    // Parámetros y argumentos
    ParamList, ParamListNonEmpty, Param,
    ArgList,   ArgListNonEmpty,

    // Tipos
    TypeName, TypeArgs, TypeArgList,
    InheritClause,

    // Cuerpo de tipos
    TypeBody, TypeMemberList, TypeMember,
    AttributeDef, MethodDef,

    // Protocolos
    ProtocolBody, ProtocolMemberList, ProtocolMember,
    MethodSignature,

    // Jerarquía de expresiones (menor → mayor precedencia)
    Expr,
    AssignExpr,      // := += -= *= /= %=
    OrExpr,          // |
    AndExpr,         // &
    CompareExpr,     // == != < > <= >=
    IsAsExpr,        // is  as
    ConcatExpr,      // @  @@
    AddExpr,         // +  -
    MulExpr,         // *  /  %
    PowerExpr,       // ^  **
    UnaryExpr,       // -  !  (prefijo)
    PostfixExpr,     // ++  --
    CallOrAccess,    // f()  .id  [i]
    PrimaryExpr,     // átomos

    // Expresiones compuestas
    BlockExpr, ExprList,
    LetExpr, LetBindingList, LetBinding,
    IfExpr, ElifChain,
    WhileExpr, ForExpr,
    NewExpr,
    VectorLiteral,
}
```

> **¿Por qué hay tantos no-terminales para expresiones?**
> Cada "nivel" de la cadena `Expr → AssignExpr → OrExpr → … → PrimaryExpr`
> representa un nivel de **precedencia de operadores**.
> La gramática codifica la precedencia estructuralmente, sin necesidad
> de reglas especiales en el parser.

### `enum Symbol`

La unión de los dos tipos anteriores. Es lo que aparece en el cuerpo
de una producción y en el stack del parser:

```rust
pub enum Symbol {
    T(Terminal),      // símbolo terminal
    NT(NonTerminal),  // símbolo no-terminal
}
```

Métodos disponibles:
- `symbol.is_terminal()` → `bool`
- `symbol.is_nonterminal()` → `bool`

Todos los tipos implementan `Display` para poder imprimir la tabla LALR legiblemente:
```
[12] AddExpr → AddExpr + MulExpr
```

---

## `production.rs` — Las reglas de la gramática

### ¿Qué es una producción?

Una producción es una regla de la forma:

```
Head → body[0]  body[1]  …  body[n-1]
```

Por ejemplo:
```
AddExpr → AddExpr + MulExpr
```

Significa: *"un `AddExpr` puede estar formado por un `AddExpr`, seguido del token `+`, seguido de un `MulExpr`"*.

### `struct Production`

```rust
pub struct Production {
    pub id:   usize,         // índice único asignado por Grammar::add()
    pub head: NonTerminal,   // lado izquierdo
    pub body: Vec<Symbol>,   // lado derecho (vacío = producción ε)
}
```

Métodos importantes:

| Método | Descripción |
|--------|-------------|
| `Production::new(head, body)` | Crea una producción con cuerpo |
| `Production::epsilon(head)` | Crea una producción vacía `head → ε` |
| `p.is_epsilon()` | `true` si el cuerpo está vacío |
| `p.body_len()` | Longitud del cuerpo — usada en REDUCE para hacer pop del stack |
| `p.symbol_at(pos)` | Símbolo en la posición `pos` del cuerpo |

### Producciones ε (epsilon)

Una producción vacía significa que el no-terminal **puede no aparecer**:

```
ElifChain → ε       significa que puede haber cero ramas elif
ParamList → ε       significa que una función puede tener cero parámetros
```

Esto es crítico para el cálculo de FIRST y FOLLOW en el algoritmo LALR.

### Display de una producción

```
[7] AddExpr → AddExpr + MulExpr
[3] ElifChain → ε
```

El número entre corchetes es el `id`. Este id es exactamente lo que
va en las celdas `REDUCE(id)` de la tabla ACTION.

---

## `mod.rs` — El struct `Grammar`

### ¿Qué es `Grammar`?

`Grammar` es el **contenedor central** de todas las producciones.
Se construye una sola vez al arrancar el compilador y se pasa por referencia
a todos los módulos que lo necesitan.

```rust
pub struct Grammar {
    pub productions: Vec<Production>,   // todas las producciones indexadas por id
    pub start: NonTerminal,             // símbolo inicial (siempre Start)
    index: HashMap<NonTerminal, Vec<usize>>, // NT → ids de sus producciones
}
```

### Cómo se construye

```rust
let mut g = Grammar::new(NonTerminal::Start);

// Cada add() asigna el id automáticamente
g.add(Production::new(NonTerminal::AddExpr, vec![
    Symbol::NT(NonTerminal::AddExpr),
    Symbol::T(Terminal::Plus),
    Symbol::NT(NonTerminal::MulExpr),
]));
```

El `id` asignado es simplemente la posición en el `Vec`: la primera producción
tiene id=0, la segunda id=1, etc. **El orden de inserción importa** porque
el builder LALR usa el id de menor valor para resolver ciertos conflictos
shift/reduce.

### Métodos de consulta

| Método | Descripción |
|--------|-------------|
| `g.add(prod)` | Registra una producción y le asigna su id |
| `g.add_all(iter)` | Añade varias producciones de golpe |
| `g.productions_for(&nt)` | `&[usize]` — ids de las producciones que expanden `nt` |
| `g.production(id)` | `&Production` — producción por id |
| `g.nonterminals()` | Iterador de todos los NT que tienen al menos una producción |
| `g.terminals()` | `Vec<Terminal>` — todos los terminales que aparecen en algún cuerpo |
| `g.len()` | Número total de producciones |
| `g.dump()` | Imprime todas las producciones (útil para depurar) |

### El índice interno

El campo `index: HashMap<NonTerminal, Vec<usize>>` permite responder
en O(1) la pregunta: *"¿qué producciones expanden `AddExpr`?"*.

Esta consulta es la más frecuente durante la construcción del AFD LALR,
porque para cada ítem `[AddExpr → AddExpr • + MulExpr, a]` necesitamos
saber todas las formas en que `AddExpr` se puede expandir.

---

## `token_translator.rs` — El puente Lexer → Parser

### El problema

El lexer produce `Token { token_type: TokenType, lexeme: String, ... }`.
El parser habla en `Terminal`. Necesitamos un puente.

### `fn token_to_terminal(token_type: &TokenType) -> Option<Terminal>`

Convierte un `TokenType` a un `Terminal`. Retorna `None` para tokens
que el parser debe ignorar completamente:

```
WHITESPACE  → None    (espacios)
COMMENT     → None    (comentarios)
NEWLINE     → None    (saltos de línea)
ERROR       → None    (ya reportado por el lexer)

KW_LET      → Some(Terminal::Let)
OP_PLUS     → Some(Terminal::Plus)
NUMBER      → Some(Terminal::Number)
EOF         → Some(Terminal::Eof)
```

### `struct TokenStream<I>`

Un iterador adaptador que envuelve el iterador de tokens del lexer:

```rust
pub struct TokenStream<I: Iterator<Item = Token>> {
    inner: I,
}
```

Cada llamada a `.next()` consume tokens del lexer interno hasta encontrar
uno que no sea ignorable y lo devuelve como `ParserToken`:

```rust
pub struct ParserToken {
    pub terminal: Terminal,   // qué tipo de símbolo es
    pub lexeme:   String,     // el texto original: "42", "myVar", "+"
    pub line:     usize,      // para reportar errores
    pub column:   usize,
}
```

### Flujo completo de un token

```
Lexer produce:
  Token { token_type: WHITESPACE, lexeme: " ", skippable: true }
  Token { token_type: KW_LET,     lexeme: "let", skippable: false }
  Token { token_type: WHITESPACE, lexeme: " ", skippable: true }
  Token { token_type: IDENTIFIER, lexeme: "x", skippable: false }

TokenStream::next() →
  loop:
    tok = WHITESPACE → skippable, continuar
    tok = KW_LET     → token_to_terminal → Some(Let) → retorna
  ParserToken { terminal: Let, lexeme: "let", line: 1, col: 1 }

TokenStream::next() →
  loop:
    tok = WHITESPACE → skippable, continuar
    tok = IDENTIFIER → token_to_terminal → Some(Identifier) → retorna
  ParserToken { terminal: Identifier, lexeme: "x", line: 1, col: 5 }
```

### ¿Cuándo se llama `token_to_terminal`?

**Dentro de cada `.next()`**, no al inicio. El parser pide el siguiente
token solo cuando lo necesita (en cada operación Shift). Esto hace que
`TokenStream` sea completamente **lazy** — no procesa el input hasta
que el parser lo demanda.

---

## `hulk_grammar.rs` — La gramática real de HULK

### `fn build() -> Grammar`

Esta función construye y devuelve la gramática completa de HULK.
La gramática tiene aproximadamente **90 producciones** agrupadas en 12 secciones.

### Sección 0 — Producción aumentada

```
Start → Program $
```

Esta es la producción especial del algoritmo LALR. Nunca la escribe
el programador — el parser la usa internamente para saber cuándo
ha terminado de parsear el programa completo (`Accept`).

### Sección 1 — Programa

```
Program → DeclList Expr
        | DeclList Expr ;
```

Un programa HULK es: cero o más declaraciones, seguidas de una expresión
global (el punto de entrada). El `;` final es opcional.

### Sección 2-3 — Declaraciones

```
DeclList → ε
         | DeclList Decl

Decl → FuncDecl
     | TypeDecl
     | ProtocolDecl
```

`DeclList → ε` permite programas sin declaraciones (solo una expresión).
La recursión izquierda `DeclList → DeclList Decl` permite cualquier
número de declaraciones.

### Sección 4 — Funciones

Cuatro variantes combinando inline/completa × con/sin tipo de retorno:

```
function tan(x) => sin(x) / cos(x);        ← inline sin retorno
function f(x) { print(x); }                ← completa sin retorno
function tan(x): Number => sin(x)/cos(x);  ← inline con retorno
function f(x): Number { ... }              ← completa con retorno
```

### Sección 5 — Parámetros

```
ParamList → ε | ParamListNonEmpty
ParamListNonEmpty → Param | ParamListNonEmpty , Param
Param → IDENTIFIER | IDENTIFIER : TypeName
```

La separación entre `ParamList` (puede ser vacío) y `ParamListNonEmpty`
(al menos un parámetro) es una técnica estándar en gramáticas LALR para
evitar ambigüedades con las comas.

### Sección 6 — Tipos

```
TypeDecl → type IDENTIFIER TypeArgs InheritClause { TypeBody }

TypeArgs      → ε | ( TypeArgList )
InheritClause → ε | inherits TypeName | inherits TypeName(ArgList)
```

Ejemplos cubiertos:
```js
type Point { ... }                              // sin args ni herencia
type Point(x, y) { ... }                        // con args
type PolarPoint inherits Point { ... }          // con herencia simple
type PolarPoint(phi, rho) inherits Point(rho * sin(phi), rho * cos(phi)) { ... }
```

### Sección 7 — Protocolos

```
ProtocolDecl → protocol IDENTIFIER { ProtocolBody }
             | protocol IDENTIFIER extends TypeName { ProtocolBody }

MethodSignature → IDENTIFIER ( ParamList ) : TypeName
```

Las firmas de protocolo **siempre** tienen anotación de tipo de retorno —
es un requisito del lenguaje, no opcional.

### Sección 8 — Nombres de tipo

```
TypeName → IDENTIFIER          (ej. Number)
         | IDENTIFIER [ ]      (ej. Number[])
         | IDENTIFIER *        (ej. Number*)
```

### Sección 9 — Jerarquía de expresiones

Esta es la sección más importante. La precedencia de operadores se codifica
en la **cadena de no-terminales**:

```
Expr
  └─ AssignExpr    (:= += -= *= /= %=)        derecha-asociativo
       └─ OrExpr   (|)                         izquierda
            └─ AndExpr  (&)                    izquierda
                 └─ CompareExpr (== != < > <= >=)
                      └─ IsAsExpr (is as)
                           └─ ConcatExpr (@ @@)
                                └─ AddExpr (+ -)
                                     └─ MulExpr (* / %)
                                          └─ PowerExpr (^ **)  derecha
                                               └─ UnaryExpr (- !)
                                                    └─ PostfixExpr (++ --)
                                                         └─ CallOrAccess
                                                              └─ PrimaryExpr
```

**¿Por qué esto codifica precedencia?**

Porque para llegar a parsear un `+` hay que "pasar por" `MulExpr`, lo que
significa que el `*` se agrupa antes que el `+`. La estructura de la gramática
*obliga* a que los operadores de mayor precedencia estén más cerca de los
átomos (`PrimaryExpr`).

**Asociatividad izquierda** se codifica con recursión izquierda:
```
AddExpr → AddExpr + MulExpr    ← izquierda: a+b+c = (a+b)+c
```

**Asociatividad derecha** se codifica con recursión derecha:
```
PowerExpr  → UnaryExpr ^ PowerExpr     ← derecha: a^b^c = a^(b^c)
AssignExpr → OrExpr := AssignExpr      ← derecha: a:=b:=c = a:=(b:=c)
```

### Sección 10 — Expresiones compuestas

| Expresión | Sintaxis |
|-----------|----------|
| Bloque | `{ e1; e2; e3 }` |
| Let | `let x = e, y: T = e in body` |
| If | `if (c) e elif (c) e else e` — el `else` es **obligatorio** |
| While | `while (c) body` |
| For | `for (x in iterable) body` |
| New | `new TypeName(args)` |

### Sección 11 — Vectores y el conflicto conocido

```
VectorLiteral → [ ]                              (vacío)
              | [ ArgListNonEmpty ]              (explícito)
              | [ Expr | IDENTIFIER in Expr ]    (generador)
```

> **⚠ Conflicto shift/reduce conocido**
>
> La forma generadora `[x^2 | x in range(0,10)]` usa `|` como separador.
> Ese mismo token es el operador OR lógico en `OrExpr → OrExpr | AndExpr`.
>
> Cuando el parser tiene `[ Expr` en el stack y ve `|` como lookahead,
> no sabe si:
> - Continuar construyendo `OrExpr → OrExpr | AndExpr` (el `|` es OR lógico)
> - Cambiar al generador `[ Expr | IDENTIFIER in Expr ]`
>
> Se resuelve en `table_builder.rs` con una regla de desambiguación:
> si el lookahead es `|` seguido del patrón `IDENTIFIER in`, elegir el generador.

---

## Flujo completo del módulo

```
hulk_grammar::build()
    │
    └─ Grammar::new(Start)
         │
         ├─ g.add(Production { id:0, head: Start, body: [Program, Eof] })
         ├─ g.add(Production { id:1, head: Program, body: [DeclList, Expr] })
         ├─ g.add(Production { id:2, head: Program, body: [DeclList, Expr, Semicolon] })
         ├─ g.add(Production { id:3, head: DeclList, body: [] })  ← ε
         ├─ ...  (~90 producciones en total)
         │
         └─ Grammar {
              productions: Vec<Production>,   // indexado por id
              index: HashMap {
                  Start    → [0],
                  Program  → [1, 2],
                  DeclList → [3, 4],
                  Expr     → [N],
                  AddExpr  → [M, M+1, M+2],
                  ...
              }
           }
```

---

## Cómo se usa desde el resto del compilador

```rust
// En el driver del parser (una sola vez al arrancar):
let grammar = parser::grammar::hulk_grammar::build();

// El builder LALR necesita la gramática para calcular FIRST/FOLLOW
let table = LalrTableBuilder::new(&grammar).build();

// Durante el parsing, el engine necesita saber qué producción aplicar:
let prod = grammar.production(reduce_id);
// pop prod.body_len() elementos del stack
// push NT(prod.head) con el estado GOTO correspondiente
```

---

## Tests disponibles

```bash
# Tests de Production
cargo test grammar::production::tests

# Tests del traductor de tokens
cargo test grammar::token_translator::tests

# Tests de cordura de la gramática de HULK
cargo test grammar::hulk_grammar::tests
```

Los tests más importantes de la gramática verifican:
- Que todos los no-terminales tienen al menos una producción
- Que los no-terminales que deben tener `ε` efectivamente la tienen
- Que la producción aumentada (`Start → Program $`) es siempre la primera (id=0)