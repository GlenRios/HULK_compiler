# Módulo `parser/lalr/`

Documentación del módulo que implementa el algoritmo LALR(1) para la generación automática de tablas de parsing.

---

## ¿Qué hace este módulo?

Este módulo es el núcleo matemático del parser. Su trabajo es tomar la gramática de HULK (definida en `grammar/`) y convertirla en dos tablas que el parser puede consultar en O(1) durante el análisis de cada token:

- **ACTION[estado][terminal]** → qué hacer cuando estamos en el estado `s` y vemos el terminal `t`
- **GOTO[estado][NT]** → a qué estado ir después de reducir por un no-terminal

El módulo no hace parsing por sí solo — genera las herramientas que el parser necesita para hacerlo. Es como construir una calculadora antes de usarla.

---

## Estructura de archivos

```
parser/lalr/
├── mod.rs           ← re-exports y punto de entrada del módulo
├── first_follow.rs  ← cálculo de conjuntos FIRST y FOLLOW
├── item.rs          ← definición de ítems LR(1) y conjuntos de ítems
├── automaton.rs     ← construcción del AFD de colecciones canónicas LR(1)
├── table_builder.rs ← fusión LR(1) → LALR(1) y llenado de las tablas
└── parse_table.rs   ← las tablas ACTION y GOTO finales
```

---

## Contexto teórico

Antes de entrar en el código, vale la pena entender el problema que estamos resolviendo.

### El problema del parsing

Dado un stream de tokens, queremos saber si forma un programa válido en HULK y, si lo es, construir su AST. El algoritmo LR hace esto manteniendo un **stack de estados** y decidiendo en cada paso una de estas cuatro acciones:

- **Shift**: consumir el token actual y pasar a un nuevo estado
- **Reduce**: aplicar una producción de la gramática, haciendo pop del stack
- **Accept**: el programa es válido, hemos terminado
- **Error**: el token no es válido en el contexto actual

La pregunta es: ¿cómo sabe el parser qué acción tomar en cada momento? La respuesta es la tabla ACTION/GOTO, que codifica todas esas decisiones de antemano.

### LR(1) vs LALR(1)

**LR(1)** construye las tablas más precisas posibles, usando el token de lookahead para decidir con más información. El problema es que genera muchísimos estados — para una gramática como HULK podría generar miles.

**LALR(1)** (Look-Ahead LR) es una optimización: fusiona los estados de LR(1) que tienen el mismo "núcleo" (mismas posiciones de punto en las producciones), combinando sus lookaheads. Esto reduce drásticamente el número de estados con muy poca pérdida de expresividad. En la práctica, la mayoría de los lenguajes de programación son LALR(1).

---

## `first_follow.rs` — Conjuntos FIRST y FOLLOW

### ¿Para qué sirven?

Estos conjuntos son la base matemática del algoritmo. Se calculan una sola vez sobre la gramática y se usan durante la construcción del autómata.

**FIRST(α)**: el conjunto de terminales que pueden aparecer al inicio de cualquier cadena derivada de α. Por ejemplo:

```
FIRST(AddExpr) = { NUMBER, IDENTIFIER, (, -, ! }
```

Esto dice: "una expresión de suma puede comenzar con un número, un identificador, un paréntesis, un menos unario, o un not".

**FOLLOW(A)**: el conjunto de terminales que pueden aparecer justo **después** del no-terminal A en alguna forma sentencial. Por ejemplo:

```
FOLLOW(Expr) = { ;, ), ], in, else, elif, $ }
```

Esto dice: "después de una expresión puede venir un punto y coma, un paréntesis de cierre, etc."

### Algoritmo de punto fijo

Ambos conjuntos se calculan iterativamente hasta que nada cambia (punto fijo). En cada iteración aplicamos las reglas:

Para **FIRST**:
- Si `A → ε`, entonces `ε ∈ FIRST(A)`
- Si `A → X₁X₂…Xₙ`, añade `FIRST(X₁) - {ε}`. Si X₁ puede derivar ε, añade también `FIRST(X₂) - {ε}`, y así sucesivamente. Si todos los Xᵢ pueden derivar ε, añade ε.

Para **FOLLOW**:
- `$ ∈ FOLLOW(Start)` (el símbolo inicial siempre tiene $ en su FOLLOW)
- Si `A → αBβ`, entonces `FIRST(β) - {ε} ⊆ FOLLOW(B)`
- Si `A → αBβ` y `ε ∈ FIRST(β)`, entonces `FOLLOW(A) ⊆ FOLLOW(B)`

### La nulabilidad es transitiva

Un punto importante que aprendimos durante la implementación: si `A → B` y `B → ε`, entonces **A también es nullable**. La propagación de nulabilidad no es solo directa — es transitiva. El algoritmo de punto fijo la captura automáticamente en sucesivas iteraciones.

### Función especial: `first_of_str_with_lookahead`

Esta función es la que más se usa durante la construcción del autómata. Dado un símbolo de lookahead `a` y una cadena de símbolos `β`, calcula `FIRST(βa)`:

```rust
ff.first_of_str_with_lookahead(&beta_symbols, &lookahead)
```

Se usa en la clausura LR(1) para determinar qué lookaheads propagar a los nuevos ítems.

---

## `item.rs` — Ítems LR(1)

### ¿Qué es un ítem LR(1)?

Un ítem LR(1) es una producción con un punto (•) que indica cuánto hemos "visto" de esa producción, más un terminal de lookahead:

```
[ A → α • β,  a ]
      ↑           ↑
    punto      lookahead
```

Por ejemplo, el ítem `[ Expr → Expr • + MulExpr, ; ]` significa:

- Ya hemos reconocido un `Expr` en el stack
- Esperamos ver `+` seguido de `MulExpr`
- Si en algún momento el punto llega al final (`Expr → Expr + MulExpr •`), podemos reducir **solo si** el lookahead es `;`

### Estructura

```rust
pub struct Item {
    pub prod_id:   usize,    // qué producción
    pub dot:       usize,    // posición del punto
    pub lookahead: Terminal, // token de lookahead
}
```

Usamos el `prod_id` en lugar de copiar la producción completa para mantener los ítems baratos de clonar. Consultamos la gramática cuando necesitamos los símbolos.

### Métodos clave

- `is_complete(&grammar)` → el punto está al final → candidato a Reduce
- `symbol_after_dot(&grammar)` → el símbolo que sigue al punto → determina el Shift o Goto
- `advance()` → nuevo ítem con el punto avanzado una posición → se usa en GOTO
- `beta_lookahead(&grammar)` → los símbolos después del punto + el lookahead → se pasa a FIRST en la clausura

### `ItemSet` — un estado del autómata

Un `ItemSet` es simplemente un conjunto de ítems LR(1). Cada estado del autómata es un `ItemSet`. El campo `core()` devuelve los ítems sin sus lookaheads — dos estados con el mismo core son candidatos a fusionarse en LALR(1).

---

## `automaton.rs` — El AFD LR(1)

### ¿Qué construye este módulo?

El Autómata Finito Determinista (AFD) de colecciones canónicas LR(1). Es un grafo donde:

- Cada **nodo** es un `ItemSet` (un estado)
- Cada **arista** etiquetada con un símbolo X va del estado I al estado `GOTO(I, X)`

El estado inicial es la clausura del ítem kernel `[Start → • Program $, $]`.

### La clausura

La clausura de un conjunto de ítems aplica esta regla hasta el punto fijo:

> Para cada ítem `[A → α • B β, a]` en el conjunto, y para cada producción `B → γ`, y para cada `b ∈ FIRST(β a)`, añade el ítem `[B → • γ, b]` al conjunto.

Intuitivamente: si esperamos ver un `B` a continuación, tenemos que considerar todas las formas en que `B` puede comenzar.

```
Estado inicial contiene: [ Start → • Program $, $ ]

Clausura añade (porque Program es un NT):
  [ Program → • DeclList Expr, $ ]
  [ Program → • DeclList Expr ;, $ ]

Clausura añade (porque DeclList es un NT):
  [ DeclList → •, $ ]      ← producción ε
  [ DeclList → • DeclList Decl, $ ]
  ...
```

### GOTO

`GOTO(I, X)` = clausura de todos los ítems de `I` que tienen `X` después del punto, con el punto avanzado:

```
GOTO({[A → α • X β, a], ...}, X) = clausura({[A → α X • β, a], ...})
```

### Construcción por BFS

El autómata se construye por BFS desde el estado inicial:

1. Calcular la clausura del ítem kernel inicial → Estado 0
2. Para cada estado en la cola de trabajo, calcular todos los GOTO posibles
3. Si el estado destino ya existe (mismo conjunto de ítems), simplemente añadir la transición
4. Si es nuevo, añadirlo a la lista de estados y a la cola

Para la gramática completa de HULK, este proceso genera aproximadamente 500-600 estados LR(1) antes de la fusión LALR.

---

## `table_builder.rs` — Fusión LR(1) → LALR(1)

### El paso de fusión

Este es el paso que define LALR(1). Dos estados LR(1) se pueden fusionar si tienen el mismo **core** (mismas producciones con mismas posiciones de punto, ignorando los lookaheads). Al fusionarlos, simplemente unimos sus conjuntos de lookaheads.

**Ejemplo**: Si tenemos los estados:

```
Estado 5:  { [A → α •, x], [B → β •, y] }
Estado 12: { [A → α •, z], [B → β •, w] }
```

Ambos tienen el mismo core `{(A→α•), (B→β•)}`, así que se fusionan en:

```
Estado 5': { [A → α •, x, z], [B → β •, y, w] }
```

### Implementación: el core como clave

Un problema técnico interesante: necesitamos usar el core como clave de un `HashMap` para detectar estados duplicados. El core es un `HashSet<(usize, usize)>`, pero `HashSet` no implementa `Hash` en Rust. La solución es convertirlo a `Vec` ordenado antes de usarlo como clave:

```rust
let mut core_key: Vec<(usize, usize)> = state.core().into_iter().collect();
core_key.sort(); // orden determinista para que cores iguales den la misma clave
core_map.get(&core_key)
```

El `.sort()` es crítico: el mismo core puede salir en distinto orden al iterar el `HashSet` en dos estados diferentes. Sin ordenar, dos estados con el mismo core no se fusionarían.

### Llenado de las tablas

Una vez fusionados los estados, llenamos las tablas:

Para cada estado `s` y cada ítem `[A → α • X β, a]` en ese estado:

- Si `X` es un **terminal** `t` y `GOTO(s, t) = s'` → `ACTION[s][t] = Shift(s')`
- Si `X` es un **no-terminal** `N` y `GOTO(s, N) = s'` → `GOTO[s][N] = s'`

Para cada ítem completo `[A → α •, a]` en el estado `s`:

- Si `A = Start` → `ACTION[s][$] = Accept`
- Si no → `ACTION[s][a] = Reduce(id de la producción A → α)`

### Resolución de conflictos

A veces dos ítems generan acciones distintas para la misma celda (estado, terminal). Esto se llama conflicto y puede ser de dos tipos:

**Shift/Reduce**: un ítem pide Shift y otro pide Reduce para el mismo token. La regla de desambiguación por defecto que implementamos es preferir Shift (que es la política correcta para la mayoría de los casos, incluyendo el `if/else` sin else colgante).

**Reduce/Reduce**: dos ítems piden Reduce con producciones distintas. La regla de desambiguación es preferir la producción de menor id (la definida primero en la gramática).

Los conflictos se registran en `table.conflicts` para que el desarrollador pueda inspeccionarlos. Una gramática LALR(1) limpia no tiene ninguno — la gramática de HULK tiene uno conocido relacionado con el vector generador `[expr | x in iter]`.

---

## `parse_table.rs` — Las tablas finales

### Estructura

```rust
pub struct ParseTable {
    pub action:    HashMap<(usize, Terminal), Action>,
    pub goto:      HashMap<(usize, NonTerminal), usize>,
    pub conflicts: Vec<ConflictKind>,
    pub num_states: usize,
}

pub enum Action {
    Shift(usize),    // ir al estado N
    Reduce(usize),   // aplicar la producción con id N
    Accept,
}
```

### Uso en runtime

En cada paso del parser, las consultas son O(1):

```rust
// Decidir qué hacer
match table.get_action(current_state, &lookahead) {
    Some(Action::Shift(s))  => { /* empujar s al stack, avanzar token */ }
    Some(Action::Reduce(p)) => { /* pop body_len(p) del stack, consultar GOTO */ }
    Some(Action::Accept)    => { /* programa válido */ }
    None                    => { /* error sintáctico */ }
}

// Después de una reducción, calcular el siguiente estado
let next = table.get_goto(top_state, &prod.head);
```

---

## Flujo completo del módulo

```
hulk_grammar::build()
        │
        ▼
FirstFollow::compute(&grammar)
   - FIRST de cada NT
   - FOLLOW de cada NT
        │
        ▼
Automaton::build(&grammar, &ff)
   - clausura del ítem inicial
   - BFS generando todos los estados y transiciones
   - ~500-600 estados LR(1) para HULK
        │
        ▼
TableBuilder::build()
   - agrupar estados por core → ~263 estados LALR(1)
   - reasignar transiciones
   - llenar ACTION y GOTO
   - registrar conflictos
        │
        ▼
ParseTable { action, goto, conflicts, num_states: 263 }
        │
        ▼  (una sola vez al arrancar el compilador)
ParserDriver::new()
        │
        ▼  (en cada token durante el parsing)
engine/parser.rs  →  AST
```

---

## Tests

```bash
# Tests de FIRST/FOLLOW sobre una gramática pequeña conocida
cargo test lalr::first_follow

# Tests del ítem LR(1) y sus operaciones
cargo test lalr::item

# Tests del autómata (verifica que se generan estados y transiciones)
cargo test lalr::automaton

# Tests del table builder (verifica que la tabla tiene las celdas correctas)
cargo test lalr::table_builder
```

Los tests de `first_follow` usan la gramática clásica `E → E+T | T | id` porque sus conjuntos FIRST y FOLLOW son conocidos y verificables a mano. Esto da confianza en que el algoritmo es correcto antes de aplicarlo a la gramática completa de HULK.

---

## Números finales para HULK

| Métrica | Valor |
|---------|-------|
| Producciones en la gramática | ~90 |
| Estados LR(1) antes de fusión | ~500-600 |
| Estados LALR(1) después de fusión | ~263 |
| Conflictos conocidos | 1 (vector generador) |
| Entradas en ACTION | ~1500 aprox |
| Entradas en GOTO | ~800 aprox |