# Codegen — Backend LLVM

Backend de generación de código del compilador HULK. Toma el AST anotado semánticamente y lo baja a LLVM IR, que se ejecuta vía JIT.

---

## Estructura de módulos

| Archivo | Responsabilidad |
|---------|-----------------|
| `mod.rs` | Re-exports públicos del módulo |
| `context.rs` | Estado global del compilador, helpers de tipos LLVM, dispatch de métodos |
| `lower_program.rs` | Orquesta la compilación; construye layouts de structs y vtables |
| `lower_decl.rs` | Compila declaraciones: tipos, métodos, constructores, funciones |
| `lower_expr.rs` | Compila expresiones: `visit_expr` — el corazón del backend |
| `objects.rs` | `ObjectRegistry` y `TypeLayout` — registro de structs LLVM por tipo |
| `symbols.rs` | `SymbolTable` con scopes apilados; `Place` — lvalue tipado |
| `value.rs` | `CgValue` — valor en tiempo de compilación |
| `runtime.rs` | Runtime embebido: declaraciones LLVM + implementaciones Rust con ABI C |
| `jit.rs` | `execute_program_jit` — punto de entrada completo (análisis → codegen → JIT) |
| `dump.rs` | `emit_ir_string` — vuelca el IR como texto para debug |
| `error.rs` | `CodegenError`, `CodegenResult` |
| `visitor.rs` | Traits `ProgramVisitor`, `DeclVisitor`, `ExprVisitor` |

---

## Flujo de compilación

```
Program (AST anotado)
        │
        ▼ visit_program()
  register_runtime()          ← declara funciones externas en el módulo LLVM
  predeclare_functions()      ← pre-declara funciones HULK (permite recursión mutua)
  build_type_layouts()        ← construye structs, vtables, asigna type_tags DFS
        │
        ├─► visit_decl() × N  ← compila cada declaración (funciones, tipos)
        │       ├─ lower_function_decl()
        │       └─ lower_type_decl()
        │               ├─ lower_method() × M
        │               ├─ lower_constructor()
        │               └─ init_vtable_global()
        │
        └─► visit_expr()      ← compila la expresión de entrada del programa
                │
                ▼
           __hulk_entry() → f64
```

---

## Layout de objetos en memoria

Todo objeto de un tipo declarado con `type` vive en heap con este layout:

```
campo 0  │ i32  type_tag    │  tag numérico único del tipo concreto
campo 1  │ ptr  vtable_ptr  │  puntero a la vtable del tipo real
campo 2+ │ f0, f1, f2...    │  campos heredados del padre (primero) + propios
```

Los `type_tag` se asignan en DFS pre-order sobre el árbol de herencia. Esto garantiza que todos los subtipos de `T` tengan tags en el rango contiguo `[type_tag, max_tag]`, lo que permite implementar `expr is T` como un range check O(1) sin ninguna estructura auxiliar.

---

## Vtable y despacho de métodos (`UserDefined`)

Cada tipo tiene una vtable global constante: un struct de punteros a función, un slot por método. Los slots se asignan una sola vez por nombre de método — si un hijo overridea un método del padre, ocupa **el mismo slot**, lo que hace que el dispatch sea correcto sin importar el tipo estático usado para acceder.

Llamada a `obj.m(args)` cuando `obj: UserDefined("T")` (`method_dispatch` en `context.rs`):

1. GEP → campo 1 del struct → cargar `vtable_ptr` (del objeto real en runtime).
2. Slot de `m` calculado en compile time a partir del layout de `T`.
3. GEP → `vtable_ptr[slot]` → cargar `fn_ptr`.
4. `build_indirect_call(fn_type, fn_ptr, [self, args...])` — dispatch dinámico.

---

## Despacho de protocolos (`Protocol`)

Cuando el tipo estático del receptor es `Protocol("P")`, la vtable no sirve — los slots de un mismo método difieren entre tipos conformantes. Se usa un **switch sobre el `type_tag`** leído en runtime:

```llvm
; b.measure()  donde  b: Measurable

%tag = load i32, ptr %b

switch i32 %tag, label %proto_unreachable [
  i32 3, label %proto_case_MBox
  i32 5, label %proto_case_Cylinder
]

proto_case_MBox:
  %r1 = call f64 @__hulk_method_MBox_measure(ptr %b)
  br label %proto_merge

proto_case_Cylinder:
  %r2 = call f64 @__hulk_method_Cylinder_measure(ptr %b)
  br label %proto_merge

proto_unreachable:
  unreachable                   ; el semantic checker garantizó conformancia

proto_merge:
  %result = phi f64 [ %r1, %proto_case_MBox ], [ %r2, %proto_case_Cylinder ]
```

LLVM convierte el switch en una jump table — mismo costo asintótico que un vtable. El bloque `unreachable` es correcto porque el semantic checker verificó en compile time que el tipo real conforma el protocolo.

Implementado en `method_dispatch_protocol()` dentro de `context.rs`.

---

## Constructores

`new T(a, b)` genera en `lower_constructor()`:

1. `malloc(sizeof(%T))` → puntero raw al objeto.
2. Escribir `type_tag` en campo 0.
3. Escribir `vtable_ptr` (global constante del tipo) en campo 1.
4. Meter parámetros del constructor en la symbol table (los inicializadores de atributos los referencian).
5. Por cada `Attribute` en el AST, evaluar el inicializador y escribirlo en el campo con `field_place()` + `store_place()`.
6. Retornar el puntero.

---

## Herencia en campos y métodos

`build_type_layouts()` usa dos helpers recursivos:

- `collect_field_names(T)` — campos del padre primero (recursivo), luego los propios en orden del AST. Garantiza que el layout del hijo sea un superconjunto prefijado del padre, lo que permite que código que maneja un objeto como `Padre*` acceda correctamente a sus campos.
- `collect_method_names(T)` — slots del padre primero; un método override no añade slot nuevo, ocupa el del padre. Métodos nuevos se añaden al final.

---

## `base()` — llamada estática al padre

`base(args)` dentro de un método `m` de tipo `T`:

1. En el semantic checker: busca `m` en el padre de `T`. Si no existe, intenta contra el constructor del padre (permite `base(args)` en métodos que inicializan el padre).
2. En el codegen (`lower_expr.rs`): llamada **directa por nombre** a `__hulk_method_<padre>_<m>` — no carga ningún vtable. Pasa `self_ptr` como primer argumento.

---

## `is` — type check en runtime

```hulk
expr is T
```

1. Evaluar `expr` → `obj_ptr`.
2. Cargar `type_tag` desde `obj_ptr[0]`.
3. Range check: `min_tag <= tag <= max_tag` donde `[min_tag, max_tag]` es el rango DFS de `T`.
4. Resultado: `i1` booleano.

Costo O(1), sin recorrido de jerarquía en runtime.

---

## `CgValue` — representación de valores

```rust
pub enum CgValue<'ctx> {
    Number(FloatValue<'ctx>),   // f64  — Number
    Bool(IntValue<'ctx>),       // i1   — Boolean
    Str(PointerValue<'ctx>),    // ptr  — String (null-terminated)
    Object(PointerValue<'ctx>), // ptr  — tipo de usuario o Object
    Vector(PointerValue<'ctx>), // ptr  — Vector [i64 count][elems...]
    Null,                       // ptr null
    Void,                       // sin valor (while, bloque vacío)
}
```

---

## `Place` — lvalue tipado

```rust
pub struct Place<'ctx> {
    pub ptr:     PointerValue<'ctx>,
    pub hulk_ty: HulkType,
}
```

Todo load y store pasa por `load_place()` / `store_place()`, que usan `hulk_ty` para elegir el tipo LLVM correcto (`f64`, `i1`, o `ptr`). Centraliza la lógica y evita cargar un `f64` como `ptr` o viceversa.

---

## Runtime embebido

Funciones definidas en `runtime.rs` con `#[unsafe(no_mangle)] extern "C"`. El JIT las registra con `add_global_mapping` en `execute_program_jit()`.

| Función | Firma C | Descripción |
|---------|---------|-------------|
| `hulk_print` | `(ptr) → void` | Imprime string + newline |
| `hulk_rand` | `() → f64` | Número aleatorio [0.0, 1.0] |
| `hulk_str_from_number` | `(f64) → ptr` | f64 a string (sin `.0` si es entero) |
| `hulk_str_concat` | `(ptr, ptr) → ptr` | Operador `@` |
| `hulk_str_concat_space` | `(ptr, ptr) → ptr` | Operador `@@` |
| `hulk_str_size` | `(ptr) → f64` | Longitud del string |
| `hulk_vec_alloc` | `(i32, i32) → ptr` | Alloca `[i64 count][8B × count]` |
| `hulk_vec_get` | `(ptr, i32, i32) → ptr` | Puntero al elemento i |
| `hulk_vec_size` | `(ptr) → f64` | Lee count como f64 |
| `hulk_range_alloc` | `(f64, f64) → ptr` | Alloca `[f64 start][f64 end][f64 current]` |
| `hulk_range_next` | `(ptr) → i1` | Avanza current, retorna `current < end` |
| `hulk_range_current` | `(ptr) → f64` | Valor actual del iterador |

---

## Tests de integración

Los tests en `tests.rs` construyen el AST directamente, lo pasan por `execute_program_jit()` y verifican el resultado numérico (46 tests en total). Cubren:

- Strings: `print`, concatenación `@` / `@@`, variables string, reasignación
- Builtins matemáticos: `sqrt`, `sin`, `cos`, `exp`, `log`, `rand`
- Potencia (`^`)
- Booleanos y comparación (`==`, `<`, `>`, etc.)
- `if / elif / else` con PHI node tipado
- Vectores explícitos: indexado, primer y último elemento
- `for` + `range(start, end)`
- Generadores de vector `[expr || x in iter]`
- **Dispatch de protocolos**: un conformante y múltiples conformantes
