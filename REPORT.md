# Compilador HULK — Informe de Arquitectura

## 1. Introducción

Este documento describe la arquitectura, las decisiones de diseño, las características implementadas y las limitaciones conocidas de un compilador completo para el lenguaje de programación HULK, construido desde cero en Rust. HULK es un lenguaje de tipado estático y orientado a expresiones, con soporte para programación orientada a objetos, incluyendo herencia, polimorfismo y protocolos. El compilador genera código máquina nativo para x86-64 utilizando LLVM como backend, mediante los bindings `inkwell` para la API de C de LLVM.

El pipeline del compilador consta de cinco etapas independientes: análisis léxico, análisis sintáctico, análisis semántico, generación de código intermedio y emisión de código nativo. Cada etapa está implementada como un módulo Rust independiente con interfaces bien definidas, lo que facilita la testeabilidad y el mantenimiento.

---

## 2. Análisis Léxico

### 2.1 Arquitectura: Lexer basado en NFA

El lexer está implementado utilizando la construcción de Thompson para convertir expresiones regulares en un Autómata Finito No Determinista (NFA), que luego se usa directamente para la tokenización — sin convertir previamente a un DFA. Este enfoque se eligió porque mantiene la implementación sencilla (la construcción del NFA es lineal en el tamaño de la regex) y al mismo tiempo es lo suficientemente eficiente para archivos fuente típicos.

Cada tipo de token se define mediante una expresión regular y una prioridad. La estructura `MasterNFA` fusiona todos los NFAs individuales de cada token en un único NFA mediante transiciones epsilon desde un estado inicial compartido. Durante la tokenización, el motor ejecuta el NFA simultáneamente por todos los caminos posibles (construcción del subconjunto en tiempo de ejecución) y selecciona la coincidencia más larga; los empates se rompen por prioridad del token.

### 2.2 Conjunto de Tokens

El lexer reconoce todos los tokens de HULK:
- **Palabras clave**: `let`, `in`, `if`, `elif`, `else`, `while`, `for`, `function`, `type`, `new`, `self`, `inherits`, `protocol`, `extends`, `is`, `as`, `true`, `false`
- **Operadores**: `+`, `-`, `*`, `/`, `%`, `^`, `**`, `@`, `@@`, `==`, `!=`, `<`, `>`, `<=`, `>=`, `&&`, `||`, `!`, `:=`, `=>`
- **Delimitadores**: `(`, `)`, `{`, `}`, `[`, `]`, `,`, `;`, `:`, `.`
- **Literales**: números (punto flotante), literales de cadena, booleanos
- **Identificadores**

Los caracteres no reconocidos producen un token `ERROR` en lugar de abortar, lo que permite al compilador reportar varios errores léxicos en una sola pasada. El formato de reporte de errores sigue el contrato de la interfaz: `(línea,columna) LEXICAL: mensaje`.

---

## 3. Análisis Sintáctico

### 3.1 Parser LALR(1) desde cero

El parser está construido sobre un generador de tablas LALR(1) escrito a mano. En lugar de usar un generador de parsers (como YACC/Bison o LALRPOP), toda la maquinaria LALR está implementada en Rust:

1. **Definición de la gramática** (`src/parser/grammar/`): La gramática de HULK está escrita como una estructura de datos (no como un DSL) usando tipos `Production` y `Symbol`. Esto hace que la gramática sea inspeccionable y testeable en tiempo de ejecución.
2. **Cálculo de FIRST/FOLLOW** (`src/parser/lalr/first_follow.rs`): Calcula los conjuntos FIRST y FOLLOW para todos los no terminales usando el algoritmo estándar de punto fijo.
3. **Conjuntos de ítems LR(0) y construcción de la tabla LALR** (`src/parser/lalr/`): Construye la colección canónica de conjuntos de ítems LR(0), calcula los conjuntos de lookahead mediante el algoritmo de fusión LALR y produce las tablas de parseo ACTION/GOTO.
4. **Motor del parser** (`src/parser/engine/`): Ejecuta el algoritmo shift-reduce sobre el flujo de tokens, invocando acciones semánticas para construir el AST en cada reducción.

Las tablas se calculan una vez al inicio y se cachean durante todo el ciclo de vida del proceso.

### 3.2 Aspectos clave de la gramática

La gramática de HULK es ambigua en su especificación original (la precedencia de operadores es implícita). La gramática del parser resuelve la ambigüedad mediante precedencia codificada directamente en la jerarquía gramatical:

```
expr → assignment
assignment → or_expr (':=' assignment)?
or_expr → and_expr ('||' and_expr)*
and_expr → equality ('&&' equality)*
equality → comparison (('==' | '!=') comparison)*
comparison → concat (('<' | '>' | '<=' | '>=') concat)*
concat → additive (('@' | '@@') additive)*
additive → term (('+' | '-') term)*
term → power (('*' | '/' | '%') power)*
power → unary ('^' power)?
unary → ('!' | '-') unary | postfix
postfix → atom ('.' id '(' args ')' | '.' id | '[' expr ']')*
```

Esto produce la precedencia y asociatividad correcta de operadores sin reglas adicionales de desambiguación.

### 3.3 Árbol de Sintaxis Abstracta

El AST está definido en `src/parser/ast/` y cubre todas las construcciones de HULK:
- **Declaraciones**: `FuncDecl`, `TypeDecl`, `ProtocolDecl`
- **Expresiones**: literales, variables, operaciones binarias/unarias, `let`-`in`, `if`-`elif`-`else`, `while`, `for`, bloques, llamadas a funciones, llamadas a métodos, acceso a campos, `new`, `is`, `as`, literales de vector, expresiones de indexado, asignación destructiva

Cada nodo del AST lleva un `Span` (posición en el código fuente: línea y columna) que se propaga hasta los mensajes de error en todas las fases posteriores.

---

## 4. Análisis Semántico

### 4.1 Type Checker en dos pasadas

El analizador semántico opera en dos pasadas sobre la lista de declaraciones:

**Pasada 1 — Recolección de declaraciones**: Se registran todas las firmas de funciones y declaraciones de miembros de tipos en una estructura de datos `TypeHierarchy`. Los miembros sin anotación reciben un placeholder `HulkType::Unknown`. Esto asegura que funciones y tipos mutuamente recursivos puedan resolverse sin restricciones de orden.

**Pasada 2 — Chequeo de cuerpos**: Cada cuerpo de función y cada inicializador/método de tipo es chequeado tipológicamente. El tipo inferido se compara con cualquier anotación declarada. Crucialmente, los tipos inferidos son **escritos de vuelta** en la `TypeHierarchy` tras la inferencia, reemplazando los placeholders `Unknown` de la Pasada 1. Esto es esencial para una generación de código correcta: si un campo `val = start` no tiene anotación, la Pasada 1 guarda `Unknown`; la Pasada 2 infiere `Number` desde la expresión de inicialización; la escritura de vuelta hace que `Number` esté disponible para el codegen.

### 4.2 Sistema de Tipos

El sistema de tipos es nominal con polimorfismo de subtipos:
- Tipos primitivos: `Number` (f64), `Boolean` (i1), `String` (ptr), `Null`
- Tipos definidos por el usuario con herencia simple
- Protocolos (tipos de interfaz estructurales)
- Vectores (`[T]`), rangos (iterable built-in)
- `HulkType::Unknown` como placeholder durante la recolección de tipos
- `HulkType::Never` para propagación de errores (evita cascadas de falsos errores)

El predicado `conforms` implementa el subtipado: `A conforma a B` si `A == B`, o si `A` es un subtipo directo o indirecto de `B`, o si `A` implementa el protocolo `B`.

### 4.3 Funciones Built-in

El analizador semántico pre-registra las siguientes funciones built-in:
- `print(x)` — imprime cualquier valor; retorna `Null`
- `sqrt(x: Number): Number`, `sin(x): Number`, `cos(x): Number`, `exp(x): Number`
- `rand(): Number` — número aleatorio uniforme en [0,1)
- `range(start, end): Range` — crea un rango iterable
- `len(v): Number`, `size(v): Number` — tamaño de vector/rango

Las constantes matemáticas `PI` y `E` están pre-declaradas como globales de tipo `Number`.

### 4.4 Reporte de Errores

Todos los errores semánticos se acumulan en un `Vec<SemanticError>` y se reportan al finalizar la pasada completa, de modo que el usuario vea todos los errores en una sola compilación en lugar de solo el primero. Cada `SemanticError` lleva un `Span` para localizar con precisión la posición en el código fuente.

---

## 5. Generación de Código

### 5.1 LLVM IR vía inkwell

El backend genera Representación Intermedia (IR) de LLVM utilizando `inkwell`, los bindings Rust-safe a la API de C de LLVM 17. El IR es luego optimizado y compilado a código máquina nativo.

La estructura `CodegenContext` mantiene todo el estado de LLVM: el `Context`, el `Module`, el `Builder` y mapas auxiliares (pila de scopes, registro de tipos, jerarquía de tipos, etc.).

### 5.2 Representación de Valores

El sistema de tipos de HULK requiere tipos LLVM distintos para valores distintos:

| Tipo HULK | Tipo LLVM | Variante CgValue |
|-----------|-----------|-----------------|
| `Number`  | `double`  | `Number(FloatValue)` |
| `Boolean` | `i1`      | `Bool(IntValue)` |
| `String`  | `ptr`     | `Str(PointerValue)` |
| Tipo de usuario | `ptr` a struct | `Object(PointerValue)` |
| Vector    | `ptr` a buffer en heap | `Vector(PointerValue)` |
| `Null`    | `ptr` null | `Null` |

El enum `CgValue` rastrea el tipo en tiempo de ejecución junto al valor LLVM, lo cual permite coerciones correctas (por ejemplo, `Bool → f64` para el retorno del entry JIT, o `Number → ptr` para pasar a `print` polimórfico).

### 5.3 Layout de Objetos

Cada tipo definido por el usuario se representa como una struct LLVM con los siguientes campos:

```
{ i32 type_tag, ptr vtable_ptr, field0, field1, ... }
```

- **`type_tag`**: Un entero único asignado en tiempo de compilación según un recorrido DFS pre-orden del árbol de herencia. Como los subtipos reciben tags contiguos después de sus ancestros, el test de tipo `is` compila a una comprobación de rango: `tag >= parent_min_tag && tag <= parent_max_tag`.
- **`vtable_ptr`**: Apunta a la vtable del tipo concreto del objeto.
- **Campos**: Todos los campos de los tipos ancestros aparecen primero (en orden de ancestro), seguidos de los campos propios del tipo.

Las vtables son structs globales de LLVM con punteros a función. Cada slot corresponde a un método; el override reemplaza el puntero en el mismo slot. La invocación de método consiste en un único load desde la vtable seguido de una llamada indirecta.

### 5.4 Herencia y Polimorfismo

Resolución de métodos durante la generación de código:

1. **Despacho estático**: Para tipos concretos conocidos (llamadas no polimórficas), la función del método se llama directamente por nombre (`__hulk_method_<Type>_<method>`).
2. **Despacho virtual**: Para expresiones de tipo interfaz/base, se carga la vtable desde el puntero al objeto y se indexa el slot del método por nombre.

La vtable se inicializa en el inicializador global de cada tipo, poblada con punteros a las implementaciones concretas.

### 5.5 Optimización

El IR se optimiza usando el nuevo pass manager de LLVM con el pipeline:

```
mem2reg,reassociate,simplifycfg
```

- **mem2reg**: Promueve patrones `alloca`/`store`/`load` a registros SSA, habilitando optimizaciones posteriores.
- **reassociate**: Reordena expresiones para mejorar el plegado de constantes.
- **simplifycfg**: Elimina ramas muertas y simplifica el grafo de flujo de control.

---

## 6. Biblioteca de Runtime

El runtime es una biblioteca C (`runtime/hulk_runtime.c`) compilada a `hulk_runtime.a` y enlazada en cada ejecutable HULK. Proporciona:

- **`hulk_print`**: Imprime una cadena seguida de un salto de línea en stdout.
- **`hulk_str_from_number`**: Convierte un `f64` a una cadena C. Si el número es entero (`.fract() == 0` y dentro del rango i64), lo formatea sin punto decimal (`42` en lugar de `42.0`).
- **`hulk_str_concat`** / **`hulk_str_concat_space`**: Concatenación de cadenas para `@` y `@@`.
- **`hulk_str_eq`**, **`hulk_str_size`**: Comparación y longitud de cadenas.
- **`hulk_vec_alloc`**, **`hulk_vec_get`**, **`hulk_vec_size`**: Operaciones sobre vectores asignados en heap.
- **`hulk_range_alloc`**, **`hulk_range_next`**, **`hulk_range_current`**: Iterador de rangos para bucles `for`.
- **`hulk_rand`**: Generador de números aleatorios uniformes.
- **`hulk_type_error`**: Manejador de errores de tipo en tiempo de ejecución.

---

## 7. Pipeline de Compilación AOT

Cuando se invoca `./hulk source.hulk`:

1. Lexer, parser y chequeo de tipos sobre el código fuente.
2. Se genera el IR de LLVM para todo el programa.
3. Se añade una `main()` C-compatible que llama a `__hulk_entry()` (el wrapper de la expresión de nivel superior) y retorna 0.
4. Se optimiza el IR.
5. Se emite un archivo objeto nativo a `/tmp/hulk_program.o` mediante `TargetMachine::write_to_file`.
6. Se enlaza con `hulk_runtime.a` usando el `gcc` del sistema: `gcc /tmp/hulk_program.o hulk_runtime.a -o ./output -lm`.

El `./output` resultante es un binario ELF nativo, autocontenido, para Linux x86-64, sin dependencias en LLVM en tiempo de ejecución.

---

## 8. Características Implementadas

Las siguientes características del lenguaje HULK están completamente implementadas:

- Todos los operadores aritméticos: `+`, `-`, `*`, `/`, `%`, `^` (potencia)
- Operadores de cadena: `@` (concatenación), `@@` (concatenación con espacio)
- Operadores de comparación y lógicos: `==`, `!=`, `<`, `>`, `<=`, `>=`, `&&`, `||`, `!`
- `let`-`in` con múltiples bindings simultáneos
- Asignación destructiva `:=` para variables y campos de objetos
- Expresiones `if`-`elif`-`else`
- Bucles `while`
- Bucles `for (x in iterable)` con `range(start, end)`
- Funciones nombradas con anotaciones de tipo opcionales (inferencia completa de tipos para parámetros y retornos sin anotar)
- Declaraciones de tipos con parámetros de constructor, campos anotados y sin anotar
- Definiciones de métodos con el shortcut `=>` y con cuerpos en bloque
- `self` dentro de métodos e inicializadores
- Herencia simple vía `inherits`
- Palabra clave `base(...)` para llamar a constructores del padre
- Despacho virtual de métodos (polimorfismo)
- Test de tipo `is` (compilado a una comprobación eficiente de rango sobre type tags)
- Cast de tipo `as` (downcast)
- Protocolos con firmas de métodos
- Chequeo de conformidad de protocolos
- Literales de vector `[a, b, c]` y expresiones generadoras `[expr || x in range]`
- Indexación de vectores `v[i]`
- Funciones built-in: `print`, `sqrt`, `sin`, `cos`, `exp`, `rand`, `range`, `len`, `size`
- Literales de tipo `String`, `Number`, `Boolean`
- Literales `true` y `false`
- Constantes matemáticas (disponibles como nombres globales)

---

## 9. Limitaciones Conocidas

- **Lambdas**: Las expresiones de función anónima (`(x) => x * 2`) aún no están soportadas.
- **Secuencias de escape en cadenas**: `\n`, `\t`, `\"` dentro de literales de cadena no se procesan; el backslash y el carácter siguiente aparecen literalmente.
- **`toString()`**: Llamar a `print(obj)` donde `obj` es un tipo definido por el usuario llama al método `toString()` del objeto si está definido, pero si no hay `toString()` sobreescrito, imprime una representación de fallback en lugar de una cadena formateada.
- **Gestión de memoria**: El compilador utiliza una estrategia simple de asignación bump/malloc sin garbage collection. Programas de larga ejecución con muchas asignaciones de cadenas u objetos perderán memoria.
- **Recuperación de errores**: El parser se detiene en el primer error sintáctico. Múltiples errores sintácticos en el mismo archivo no se reportan en una sola pasada.
- **Aritmética entera**: HULK usa `Number` (IEEE 754 double) para todos los valores numéricos. No existe un tipo entero separado, por lo que enteros muy grandes pueden perder precisión.

---

## 10. Decisiones de Diseño y Compromisos

**¿Por qué LALR(1) desde cero?** Usar un generador de parsers escrito a mano hizo que la gramática y la construcción de tablas fueran completamente transparentes y depurables. Además elimina la dependencia de herramientas externas, lo que simplifica el proceso de build.

**¿Por qué LLVM / inkwell?** LLVM provee un backend maduro y altamente optimizado para x86-64. Usar `inkwell` (bindings Rust-safe a LLVM) evita los peligros del FFI crudo a C dando acceso al pipeline completo de optimización de LLVM.

**¿Por qué linkeo dinámico con `force-dynamic` para LLVM?** El linkeo estático contra LLVM produce ejecutables de 50–200 MB. El linkeo dinámico contra `libLLVM-17.so` mantiene el binario del compilador pequeño y comparte la biblioteca de LLVM con otras herramientas del sistema.

**¿Por qué una biblioteca de runtime en C?** El runtime maneja operaciones que son difíciles de implementar puramente en IR de LLVM: impresión formateada, asignación de cadenas, vectores en heap y el iterador de rangos. Separar estas operaciones en una biblioteca C (`hulk_runtime.a`) las hace fáciles de inspeccionar, testear y reemplazar.

**¿Por qué type tags en orden DFS pre-orden para `is`?** El operador `is` es muy común en código OOP de HULK. Asignar type tags en orden DFS pre-orden del árbol de herencia garantiza que todos los subtipos de un tipo dado tengan tags contiguos, convirtiendo el chequeo `is` en una única comparación de rango — O(1) en tiempo de ejecución sin necesidad de una búsqueda en vtable ni recorrido del objeto clase.

---

## 11. Tests

El compilador incluye una suite de tests (`cargo test`) con 384 tests que cubren:
- Lexer: tokenización de todos los tipos de tokens, casos límite, tokens de error
- Parser: parseo de todas las producciones de la gramática, verificación de la forma del AST
- Análisis semántico: chequeo de tipos, detección de errores, conformidad de protocolos
- Generación de código y ejecución JIT: aritmética, control de flujo, funciones, OOP, vectores, rangos, built-ins

Los 384 tests pasan en la plataforma de desarrollo de referencia (Ubuntu 22.04 con LLVM 17).
