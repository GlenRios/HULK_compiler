# Análisis Semántico — HULK Compiler

Módulo de análisis semántico del compilador HULK. Recibe el AST producido por el
parser y verifica que el programa sea semánticamente correcto: tipos consistentes,
variables definidas, herencia válida, protocolos cumplidos, entre otras reglas.

## Estructura de archivos

```
src/semantic/
├── mod.rs           ← Punto de entrada público del módulo
├── errors.rs        ← Definición de todos los errores semánticos
├── symbol_table.rs  ← Tabla de símbolos con scopes anidados
├── type_system.rs   ← Jerarquía de tipos, conformance, LCA
└── type_checker.rs  ← Visitor principal sobre el AST
```

---

## Archivo: `mod.rs`

### ¿En qué consiste?

Módulo organizador y punto de entrada público del análisis semántico. Declara los
submódulos, reexporta los tipos más usados y expone la función `analyze` que
orquesta todo el proceso.

### Función general

Permite al resto del compilador invocar el análisis semántico con una sola llamada:

```rust
match semantic::analyze(&program) {
    Ok(())      => println!("✅ Semántico OK"),
    Err(errors) => { for e in &errors { eprintln!("❌ {}", e); } }
}
```

### Componentes

`pub mod` — declara los cuatro submódulos del analizador.

`pub use` — reexporta `SemanticError`, `TypeChecker` y `HulkType` para que el
resto del proyecto los use sin rutas largas.

`analyze(program)` — función pública principal. Crea un `TypeChecker`, ejecuta
`check_program` y retorna `Ok(())` si no hay errores, o `Err(Vec<SemanticError>)`
con todos los errores encontrados.

`#[cfg(test)] mod tests` — incluye el módulo de tests únicamente en compilaciones
de prueba.

---

## Archivo: `errors.rs`

### ¿En qué consiste?

Define la enumeración `SemanticError` con todas las variantes de error que el
analizador puede detectar. Cada variante incluye la información necesaria para
reportar al usuario dónde ocurrió el problema y qué estuvo mal.

### Función general

Centralizar en un solo lugar todos los errores semánticos posibles del lenguaje
HULK. Todos los errores llevan un `Span` para poder reportar línea y columna exactas.
Implementa `Display` para producir mensajes en español listos para el usuario.

### Categorías de errores

**Variables y scope:**

`UndefinedVariable` — uso de una variable que no está definida en ningún scope visible.
`UndefinedFunction` — llamada a una función que no existe.
`UndefinedType` — referencia a un tipo o protocolo no declarado.
`Redefinition` — intento de declarar un nombre que ya existe en el mismo scope.

**Tipos:**

`TypeMismatch` — el tipo encontrado no conforma con el tipo esperado. Incluye el
nombre del tipo esperado y el encontrado.
`CannotInferType` — no se puede inferir el tipo de una variable porque su valor
tiene tipo `Unknown`.
`InheritFromPrimitive` — un tipo intenta heredar de `Number`, `String` o `Boolean`,
lo cual está prohibido en HULK.
`CircularInheritance` — la cadena de herencia de un tipo forma un ciclo.

**Llamadas:**

`WrongArgCount` — una llamada (función, constructor, `base()`) recibe un número
distinto de argumentos al esperado.
`NotCallable` — se intenta invocar una expresión que no es función ni tipo.
`MethodNotFound` — se llama a un método que no existe en el tipo del objeto ni en
sus ancestros.
`AttributeNotFound` — se accede a un atributo que no existe en el tipo del objeto.

**Semántica especial:**

`SelfAssignment` — intento de asignar a `self` (prohibido en HULK).
`SelfInInitializer` — uso de `self` en la expresión de inicialización de un
atributo (prohibido porque el objeto aún no está construido).
`InvalidLValue` — el lado izquierdo de una asignación no es un lvalue válido
(solo variables, atributos e índices lo son).
`ProtocolNotConformed` — un tipo no cumple un protocolo requerido. Indica el
protocolo y el método específico que falta o tiene firma incompatible.
`OverrideMismatch` — un método sobreescrito en un tipo hijo no tiene la misma
firma que en el padre (aridad, tipos de parámetros o tipo de retorno incompatible).
`InvalidCast` — se intenta un cast entre dos tipos sin ninguna relación de herencia.

**Operadores:**

`InvalidOperandType` — un operador unario o postfijo se aplica a un tipo incorrecto
(por ejemplo, `-` sobre un `Boolean`).
`InvalidBinaryTypes` — un operador binario se aplica a tipos incompatibles (por
ejemplo, `+` entre `String` y `Boolean`).

### Métodos

`span(&self) -> Span` — retorna el span de cualquier variante de error para
localizarlo en el código fuente.

`Display` — implementado para todos los errores, produce mensajes como:
`[3:10] Variable 'x' no definida` o `[7:2] 'Dog' no cumple el protocolo 'Printable': falta 'show'`.

---

## Archivo: `symbol_table.rs`

### ¿En qué consiste?

Implementa la tabla de símbolos con scopes anidados y búsqueda léxica. Guarda todos
los nombres visibles en cada punto del programa — variables, funciones, tipos y
protocolos — organizados en una pila de scopes.

### Función general

Resolver nombres durante el análisis semántico. Cuando el checker entra en un bloque,
función o `let`, abre un scope nuevo. Al salir, lo cierra. La búsqueda siempre va
del scope más interno al más externo, implementando scoping léxico correcto.

### `SymbolKind`

Enumeración que distingue los cuatro tipos de símbolo que pueden aparecer en un
scope:

`Variable { ty, mutable }` — variable local o parámetro. `mutable` distingue
parámetros (inmutables) de variables `let` (mutables con `:=`).

`Function { params, return_type }` — función global. Guarda los tipos de sus
parámetros y su tipo de retorno (puede ser `Unknown` si no tiene anotación y aún
no se ha inferido).

`Type` — nombre de un tipo declarado con `type`. Se registra para detectar uso
como constructor sin `new`.

`Protocol` — nombre de un protocolo declarado con `protocol`.

### `Symbol`

Estructura que agrupa nombre y `SymbolKind`. Tiene constructores estáticos de
conveniencia: `variable`, `function`, `type_sym`, `protocol_sym`.

### `SymbolTable`

Estructura principal. Contiene `scopes: Vec<HashMap<String, Symbol>>`. El scope 0
es el global (nunca se elimina).

`new()` — crea la tabla con el scope global vacío.

`push_scope()` / `pop_scope()` — abre y cierra scopes. `pop_scope` nunca elimina
el scope global.

`define(name, symbol) -> bool` — define un símbolo en el scope actual. Retorna
`false` si el nombre ya existe en ese mismo scope (redefinición), sin tocarlo.

`lookup(name) -> Option<&Symbol>` — búsqueda léxica desde el scope más interno
hacia el global. Retorna el primer símbolo que coincida.

`in_current_scope(name) -> bool` — verifica si un nombre ya está definido en el
scope actual (sin subir). Usado para detectar redefiniciones.

`update_type(name, new_ty) -> bool` — actualiza el tipo de una variable ya
definida en cualquier scope visible. Usado para inferencia.

`update_function_return(name, return_type) -> bool` — actualiza el tipo de retorno
de una función. Usado cuando el checker infiere el retorno del cuerpo de una función
sin anotación explícita.

`depth() -> usize` — profundidad actual de anidamiento (0 = global).

---

## Archivo: `type_system.rs`

### ¿En qué consiste?

Define el sistema de tipos semántico de HULK: la representación interna de los tipos
(`HulkType`), la jerarquía de herencia (`TypeHierarchy`), las reglas de conformance,
el algoritmo LCA para `if-elif-else`, y la verificación estructural de protocolos.

### Función general

Responder las preguntas clave del análisis de tipos: ¿el tipo A conforma con el tipo
B? ¿Cuál es el ancestro común más cercano de A y B? ¿El tipo T cumple el
protocolo P? Además registra todos los tipos built-in al inicializarse.

### `HulkType`

Enumeración que representa un tipo semántico ya resuelto (distinto de `TypeName`
del AST, que es solo sintaxis):

`Number`, `StringT`, `Boolean`, `Null`, `Object` — tipos primitivos y raíz.
`Vector(Box<HulkType>)` — vector de elementos de un tipo concreto.
`UserDefined(String)` — tipo declarado con `type` por el programador.
`Protocol(String)` — protocolo declarado con `protocol`.
`Unknown` — tipo aún no inferido (anotación ausente).
`Never` — tipo de expresiones con error. No propaga errores en cascada: cualquier
operación con `Never` retorna `Never` sin generar errores adicionales.

Métodos: `is_primitive()`, `is_never()`, `name() -> String`.

### `FuncSignature`

Firma de una función o método: `params: Vec<(String, HulkType)>` y
`return_type: HulkType`. Usada tanto para funciones globales como para métodos de
tipos y firmas de protocolos.

### `TypeInfo`

Información de un tipo registrado en la jerarquía:

`name`, `parent: Option<String>` — nombre y padre directo en la herencia.
`constructor_params: Vec<(String, HulkType)>` — parámetros del constructor en
orden. Permite verificar aridad y tipos en `new T(...)` y en `base(...)`.
`attributes: HashMap<String, HulkType>` — atributos con sus tipos resueltos.
`methods: HashMap<String, FuncSignature>` — métodos con sus firmas completas.
`is_builtin: bool` — distingue tipos predefinidos (`Number`, `String`, etc.) de
los declarados por el usuario.

### `ProtocolInfo`

Información de un protocolo: nombre, protocolo padre (`extends`) y métodos
requeridos con sus firmas.

### `TypeHierarchy`

Contenedor principal del sistema de tipos. Tiene `types` y `protocols` como
`HashMap`. Se inicializa con todos los built-ins.

**`conforms(child, ancestor) -> bool`** — verifica si `child` conforma con
`ancestor`. Reglas implementadas: `Never` conforma con todo (suprime cascadas),
`Unknown` conforma con todo (pendiente de inferencia), todo conforma con `Object`,
`Null` conforma con tipos de usuario, subtipado nominal entre `UserDefined`, y
conformance estructural contra protocolos. Los vectores son covariantes: `T[]`
conforma con `U[]` si `T` conforma con `U`.

**`is_subtype(child, ancestor) -> bool`** — sube la cadena de herencia nominal
verificando si `child` es descendiente de `ancestor`.

**`conforms_protocol(type_name, protocol) -> bool`** — verificación estructural
completa. Primero comprueba si el padre ya conforma (herencia automática). Luego
busca cada método del protocolo subiendo la jerarquía del tipo (los métodos
heredados cuentan). Usa varianza correcta: retorno covariante y parámetros
contravariantes. Sigue la cadena de protocolos padres recursivamente.

**`lookup_method_for_protocol(type_name, method) -> Option<FuncSignature>`** —
busca un método subiendo la jerarquía del tipo. Permite que los métodos heredados
satisfagan un protocolo.

**`signatures_protocol_compatible(type_sig, proto_sig) -> bool`** — verifica
compatibilidad de firma con varianza correcta: el tipo de retorno del método puede
ser un subtipo del retorno del protocolo (covariante), y los tipos de parámetros
del método pueden ser supertipos de los del protocolo (contravariante).

**`first_protocol_violation(type_name, protocol) -> Option<String>`** — retorna
el nombre del primer método que falla al cumplir el protocolo, incluyendo si la
firma es incompatible. Produce el mensaje específico en `ProtocolNotConformed`.

**`lca(a, b) -> HulkType`** — Lowest Common Ancestor. Calcula el ancestro común
más cercano de dos tipos. Usado para determinar el tipo de `if-elif-else`: si
`then` es `Dog` y `else` es `Cat` y ambos heredan de `Animal`, el tipo del
`if` es `Animal`. `Never` no participa en el LCA.

**`has_circular_inheritance(type_name) -> bool`** — detecta ciclos en la cadena
de herencia usando un `HashSet` de visitados.

**`register_builtins()`** — registra al inicializarse: `Object`, `Number`,
`String`, `Boolean`, `Range` con sus métodos built-in (`toString`, `size`, etc.),
y el protocolo `Iterable` con los métodos `next()` y `current()`. `Range`
implementa `Iterable` automáticamente.

---

## Archivo: `type_checker.rs`

### ¿En qué consiste?

El visitor principal del análisis semántico. Recorre el AST completo nodo por nodo,
resolviendo tipos, verificando reglas semánticas y acumulando errores. Es el
archivo más grande del módulo y orquesta el uso de todos los demás.

### Función general

Implementar el algoritmo de chequeo de tipos de HULK en tres pasos: recolección de
firmas, chequeo de cuerpos y chequeo de la expresión de entrada. Cada método
`check_*` recibe un nodo del AST y retorna el `HulkType` de esa expresión, o
acumula errores en `self.errors`.

### Estructura `TypeChecker`

`symbols: SymbolTable` — tabla de símbolos activa durante el recorrido.
`types: TypeHierarchy` — jerarquía de tipos con todos los tipos y protocolos
registrados.
`errors: Vec<SemanticError>` — errores acumulados durante el análisis.
`current_type: Option<String>` — nombre del tipo que se está analizando. Permite
a `check_base` saber cuál es el padre y a `check_identifier` detectar `self` en
contexto de método.
`current_ret_type: Option<HulkType>` — tipo de retorno esperado de la función
actual. Guardado y restaurado con `replace`/`prev_ret` para soportar funciones
anidadas.
`in_initializer: bool` — `true` mientras se chequea la expresión inicial de un
atributo. Prohíbe el uso de `self`.

### Flujo de ejecución: los 3 pasos

**`check_program(program)`** — punto de entrada. Ejecuta los tres pasos en orden
y retorna todos los errores acumulados.

**Paso 1 — `collect_all_declarations`**: hace dos pasadas sobre las declaraciones.
En la primera registra nombres de funciones, tipos y protocolos en la tabla de
símbolos y en la jerarquía (forward declaration, permite recursión mutua). En la
segunda resuelve los miembros de los tipos (atributos y métodos) ya con todos los
nombres disponibles.

**Paso 2 — chequeo de cuerpos**: recorre cada declaración y verifica su cuerpo
completo con tipos ya resueltos.

**Paso 3 — `check_expr(&program.entry)`**: chequea la expresión de entrada global.

### Recolección de firmas (Paso 1)

`collect_func(f)` — registra la función en la tabla de símbolos con sus tipos de
parámetros y retorno. Detecta redefinición.

`collect_type(t)` — registra el tipo en la jerarquía con su `constructor_params`,
padre y nombre. Verifica que no herede de primitivos y que el padre exista.

`collect_type_members(t)` — segunda pasada sobre los tipos: registra atributos y
métodos con sus firmas. Verifica override estricto: si el padre tiene el método,
la firma del hijo debe ser compatible en aridad, tipos de parámetros y tipo de
retorno (covariante).

`collect_protocol(p)` — registra el protocolo y sus firmas de métodos. Verifica
que el protocolo padre exista.

### Chequeo de declaraciones (Paso 2)

`check_func_decl(f)` — abre un scope, define los parámetros, chequea el cuerpo y
verifica que el tipo retornado conforme con la anotación. Si no hay anotación,
infiere el tipo del cuerpo y lo propaga a la tabla de símbolos con
`update_function_return` para que los llamadores vean el tipo correcto.

`check_type_decl(t)` — abre un scope con los parámetros del constructor. Detecta
herencia circular. Verifica los `parent_args` (argumentos al constructor del padre
en la cláusula `inherits`): aridad y tipos deben coincidir, y los `parent_args`
pueden referenciar los parámetros del constructor propio. Chequea atributos (con
`self` prohibido en inicializadores) y métodos (con `self` disponible).

`check_protocol_decl(p)` — las firmas ya fueron verificadas en la recolección,
no hace trabajo adicional.

### Chequeo de expresiones (Paso 3)

`check_expr(expr)` — dispatcher central que delega en el método específico según
la variante del enum `Expr`.

**Literales y átomos:**

`check_literal` — retorna el `HulkType` directamente según la variante: `Number`,
`StringT`, `Boolean` o `Null`.

`check_identifier` — busca el nombre en la tabla de símbolos. Detecta `self` en
inicializadores (error). Retorna el tipo del símbolo encontrado o `Never` si no
existe.

`check_base` — válido solo dentro de `current_type`. Retorna el `HulkType` del
padre del tipo actual.

**Operaciones:**

`check_binary` — verifica los tipos de ambos operandos según el operador. Los
operadores aritméticos requieren `Number`, los lógicos requieren `Boolean`, las
comparaciones ordenadas requieren `Number`. Los operadores `@` y `@@` aceptan
cualquier tipo y retornan `String`. `==` y `!=` requieren que los tipos sean
compatibles. Si algún operando es `Never`, retorna `Never` sin emitir errores.

`check_unary` — `-` requiere `Number`, `!` requiere `Boolean`.

`check_postfix` — `++` y `--` requieren `Number`.

**Asignaciones:**

`check_assign` — prohíbe `self` como target y verifica que el target sea un lvalue
válido (`Identifier`, `Access` o `Index`). Las asignaciones compuestas (`+=`,
`-=`, etc.) requieren `Number` en ambos lados. La asignación simple (`:=`) verifica
conformance del valor con el tipo del target, usando `emit_type_or_protocol_error`
para mensajes específicos si el target es un protocolo.

**Llamadas:**

`check_call` — tres casos: si el callee es `Expr::Base`, verifica los argumentos
contra el constructor del padre. Si el callee es un `Identifier`, busca la función
o tipo en la tabla y verifica aridad y tipos. Para cualquier otra expresión (functor
de primera clase), chequea el callee y los argumentos sin verificar firma.

`check_call_args` — helper que verifica aridad y, para cada argumento, llama a
`emit_type_or_protocol_error` que produce `ProtocolNotConformed` si el parámetro
esperado es un protocolo, o `TypeMismatch` en cualquier otro caso.

`check_method_call` — resuelve el tipo del objeto, busca el método subiendo la
jerarquía con `lookup_method`, y verifica los argumentos.

`check_access` — resuelve el tipo del objeto y busca el atributo con
`lookup_attribute`, que sube la cadena de herencia.

`check_index` — verifica que la colección sea un `Vector` y que el índice sea
`Number`. Retorna el tipo del elemento.

**Expresiones compuestas:**

`check_block` — abre un scope, chequea todas las expresiones y retorna el tipo
de la última (el bloque toma el tipo de su expresión final).

`check_let` — abre un scope. Para cada binding, chequea el valor, verifica la
anotación de tipo si existe (con `emit_type_or_protocol_error`), o infiere el tipo
del valor. Define el binding en el scope actual. Los bindings se procesan de
izquierda a derecha, cada uno puede ver los anteriores.

`check_if` — verifica que la condición sea `Boolean`. Calcula el tipo de cada rama
(`then`, `elif`s, `else`) y aplica `lca` sucesivamente para obtener el tipo final.

`check_while` — verifica que la condición sea `Boolean`. Retorna el tipo del
cuerpo.

`check_for` — determina el tipo del elemento según el tipo del iterable: si es
`Vector<T>` el elemento es `T`, si es `Range` es `Number`, si implementa
`Iterable` usa el retorno de `current()`. Define la variable de iteración en un
scope nuevo.

`check_new` — verifica que el tipo exista, que no sea primitivo, y que los
argumentos coincidan en aridad y tipos con el `constructor_params` registrado.

`check_vector` — para la forma explícita, calcula el LCA de todos los elementos.
Para la forma generadora, determina el tipo del elemento del iterable, define la
variable en scope, chequea el body y retorna `Vector<body_type>`.

`check_is` — verifica que el tipo exista. Siempre retorna `Boolean`.

`check_as` — verifica que haya relación de herencia en alguna dirección (upcast
o downcast). Retorna el tipo target. Emite `InvalidCast` si no hay ninguna
relación.

### Helpers

`emit_type_or_protocol_error(found, expected, span)` — decide entre
`ProtocolNotConformed` (cuando `expected` es un protocolo, incluyendo el método
específico faltante) y `TypeMismatch` (en cualquier otro caso).

`lookup_method(type_name, method)` — sube la cadena de herencia buscando el
método. Permite que los métodos heredados sean invocables en subtipos.

`lookup_attribute(type_name, attr)` — igual que `lookup_method` pero para
atributos.

`resolve_type_name(tn)` — convierte un `TypeName` del AST (`Simple`, `Vector`,
`Iterable`) al `HulkType` semántico correspondiente.

`resolve_opt_type(ann, span)` — convierte una anotación de tipo opcional. Si es
`None` retorna `HulkType::Unknown` para inferencia posterior.

`name_to_hulk_type(name)` — traduce un nombre de tipo (string) al `HulkType`
correspondiente, distinguiendo primitivos, `Object`, protocolos y tipos de usuario.

---

## Flujo completo

```
Program (AST)
     │
     ▼
collect_all_declarations
     ├── collect_func        → SymbolTable (función)
     ├── collect_type        → TypeHierarchy (tipo + constructor_params)
     ├── collect_protocol    → TypeHierarchy (protocolo + firmas)
     └── collect_type_members → TypeHierarchy (atributos + métodos)
     │
     ▼
check_decl  (para cada declaración)
     ├── check_func_decl    → verifica cuerpo + inferencia de retorno
     └── check_type_decl    → verifica parent_args + atributos + métodos
     │
     ▼
check_expr  (expresión de entrada)
     │
     ▼
Vec<SemanticError>  →  Ok(()) o Err(errores)
```

---

## Tipos de errores semánticos detectados

El analizador detecta los 20 errores documentados en la especificación de HULK:

redefinición de nombres, uso de variables no definidas, herencia de tipos
primitivos, herencia circular, `self` en inicializadores, `self` como target de
asignación, lvalue inválido, mismatch de tipos en asignación y en llamadas, aridad
incorrecta en funciones y constructores, método o atributo no encontrado, override
con firma incompatible, cast entre tipos sin relación, incumplimiento de protocolo
con indicación del método faltante, operadores aplicados a tipos incorrectos, e
incapacidad de inferir el tipo de una expresión.
