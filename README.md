# HULK Compiler

Compilador completo para el lenguaje **HULK** (Havana University Language for Kompilers), escrito desde cero en **Rust** con LLVM como backend. Genera ejecutables nativos para **Linux x86-64**.

HULK es un lenguaje de tipado estático y orientado a expresiones, con soporte para herencia simple, polimorfismo, protocolos y vectores.

---

## Características

- **Lexer NFA**: tokenización mediante construcción de Thompson, sin paso intermedio por DFA.
- **Parser LALR(1)**: tablas LALR construidas a mano (sin generadores tipo YACC).
- **Análisis semántico en dos pasadas**: recolección de declaraciones + chequeo de tipos con inferencia.
- **Codegen LLVM**: emisión de IR vía `inkwell`, optimización (`mem2reg`, `reassociate`, `simplifycfg`) y compilación AOT a objeto nativo.
- **Runtime en C**: biblioteca `hulk_runtime.a` enlazada estáticamente para `print`, concatenación de strings, vectores y rangos.
- **Tipos**: `Number`, `String`, `Boolean`, tipos definidos por el usuario, protocolos, vectores `[T]`.
- **OOP**: herencia (`inherits`), métodos virtuales (vtable), `is` y `as`, `base(...)`.
- **Built-ins**: `print`, `sqrt`, `sin`, `cos`, `exp`, `rand`, `range`, `len`, `size`, constantes `PI` y `E`.

---

## Requisitos

- Ubuntu 24.04 LTS (o equivalente Linux x86-64)
- `build-essential`, `llvm-17`, `llvm-17-dev`, `curl`
- Rust (instalable con `rustup`)

```bash
sudo apt-get install -y build-essential llvm-17 llvm-17-dev curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

---

## Compilar

```bash
make build
```

Esto produce dos artefactos en la raíz del repo:

- `./hulk` — el ejecutable del compilador.
- `./hulk_runtime.a` — la biblioteca de runtime, enlazada automáticamente.

---

## Uso

```bash
./hulk programa.hulk
./output
```

`./hulk` compila el fichero fuente a un binario ELF nativo llamado `./output`, que se ejecuta directamente.

### Ejemplo

```hulk
function fib(n: Number): Number =>
    if (n < 2) n else fib(n - 1) + fib(n - 2);

print(fib(10));
```

```bash
$ ./hulk fib.hulk && ./output
55
```

---

## Códigos de salida

| Código | Significado          |
|:------:|----------------------|
| 0      | Compilación exitosa  |
| 1      | Error léxico         |
| 2      | Error sintáctico     |
| 3      | Error semántico      |

Los errores se reportan a `stderr` en el formato `(línea,columna) TIPO: mensaje`.

---

## Estructura del proyecto

```
HULK_compiler/
├── src/
│   ├── lexer/        # NFA + Thompson + tokenización
│   ├── parser/       # LALR(1) + AST
│   ├── semantic/     # type checker en dos pasadas
│   ├── codegen/      # emisión LLVM IR + AOT
│   └── main.rs       # pipeline completo
├── runtime/          # hulk_runtime.c
├── tests/            # tests de integración
├── docs/             # informe formal en LaTeX
├── Makefile
├── REPORT.md         # informe técnico de arquitectura
└── README.md
```

---

## Documentación técnica

Para detalles completos de la arquitectura, decisiones de diseño, features implementadas y limitaciones, ver [`REPORT.md`](REPORT.md). También se incluye una versión formal en LaTeX en `docs/REPORT.tex`.

---

## Tests

```bash
cargo test --release
```

384 tests cubren el lexer, parser, análisis semántico y la ejecución JIT del codegen.

---

## Licencia

Ver [`LICENSE`](LICENSE).
