# HULK Compiler — Architecture Report

## 1. Introduction

This document describes the architecture, design decisions, implemented features, and known limitations of a full-stack compiler for the HULK programming language, built from scratch in Rust. HULK is a statically typed, expression-oriented language with object-oriented features including inheritance, polymorphism, and protocols. The compiler targets native x86-64 machine code via LLVM as its backend, using the `inkwell` Rust bindings to the LLVM C API.

The compiler pipeline consists of five independent stages: lexical analysis, syntactic analysis, semantic analysis, intermediate code generation, and native code emission. Each stage is implemented as a separate Rust module with well-defined interfaces, enabling testability and maintainability.

---

## 2. Lexical Analysis

### 2.1 Architecture: NFA-based Lexer

The lexer is implemented using the Thompson construction to convert regular expressions into a Non-deterministic Finite Automaton (NFA), which is then used directly for tokenization — without converting to a DFA first. This approach was chosen because it keeps the implementation simple (NFA construction is linear in the size of the regex) while being efficient enough for typical source files.

Each token type is defined by a regular expression and a priority. The `MasterNFA` struct merges all individual token NFAs into a single NFA using epsilon transitions from a shared start state. During tokenization, the engine runs the NFA simultaneously on all possible paths (subset construction at runtime) and selects the longest match; ties are broken by token priority.

### 2.2 Token Set

The lexer recognizes all HULK tokens:
- **Keywords**: `let`, `in`, `if`, `elif`, `else`, `while`, `for`, `function`, `type`, `new`, `self`, `inherits`, `protocol`, `extends`, `is`, `as`, `true`, `false`
- **Operators**: `+`, `-`, `*`, `/`, `%`, `^`, `**`, `@`, `@@`, `==`, `!=`, `<`, `>`, `<=`, `>=`, `&&`, `||`, `!`, `:=`, `=>`
- **Delimiters**: `(`, `)`, `{`, `}`, `[`, `]`, `,`, `;`, `:`, `.`
- **Literals**: numbers (floating-point), string literals, booleans
- **Identifiers**

Unrecognized characters produce an `ERROR` token rather than aborting, allowing the compiler to report multiple lexical errors in a single pass. The error reporting format follows the interface contract: `(line,col) LEXICAL: message`.

---

## 3. Syntactic Analysis

### 3.1 LALR(1) Parser from Scratch

The parser is built on a hand-written LALR(1) parse table generator. Rather than using a parser generator tool (like YACC/Bison or LALRPOP), the entire LALR machinery is implemented in Rust:

1. **Grammar definition** (`src/parser/grammar/`): The HULK grammar is written as a data structure (not a DSL) using `Production` and `Symbol` types. This makes the grammar inspectable and testable at runtime.
2. **FIRST/FOLLOW computation** (`src/parser/lalr/first_follow.rs`): Computes the FIRST and FOLLOW sets for all non-terminals using the standard fixed-point algorithm.
3. **LR(0) item sets and LALR table construction** (`src/parser/lalr/`): Builds the canonical collection of LR(0) item sets, computes lookahead sets via the LALR merging algorithm, and produces the ACTION/GOTO parse tables.
4. **Parser driver** (`src/parser/engine/`): Runs the shift-reduce algorithm over the token stream, invoking semantic actions to build the AST on every reduction.

The tables are computed once at startup and cached for the lifetime of the process.

### 3.2 Grammar Highlights

HULK's grammar is ambiguous in its original specification (operator precedence is implicit). The parser grammar resolves ambiguity via precedence climbing encoded directly in the grammar hierarchy:

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

This produces the correct operator precedence and associativity without any additional disambiguation rules.

### 3.3 Abstract Syntax Tree

The AST is defined in `src/parser/ast/` and covers all HULK constructs:
- **Declarations**: `FuncDecl`, `TypeDecl`, `ProtocolDecl`
- **Expressions**: literals, variables, binary/unary operations, `let`-`in`, `if`-`elif`-`else`, `while`, `for`, blocks, function calls, method calls, field access, `new`, `is`, `as`, vector literals, index expressions, destructive assignment

Every AST node carries a `Span` (source position: line and column) which propagates through to error messages in all subsequent phases.

---

## 4. Semantic Analysis

### 4.1 Two-Pass Type Checker

The semantic analyzer operates in two passes over the declaration list:

**Pass 1 — Declaration collection**: All function signatures and type member declarations are registered into a `TypeHierarchy` data structure. Unannotated members receive a `HulkType::Unknown` placeholder. This ensures that mutually recursive functions and types can be resolved without ordering constraints.

**Pass 2 — Body checking**: Each function body and type member initializer/method body is type-checked. The inferred type is compared against any declared annotation. Crucially, inferred types are **written back** to the `TypeHierarchy` after inference, replacing the `Unknown` placeholders from Pass 1. This is essential for correct code generation: if a field `val = start` has no annotation, Pass 1 stores `Unknown`; Pass 2 infers `Number` from the initializer expression; the write-back makes `Number` available to the codegen.

### 4.2 Type System

The type system is a nominal type system with subtype polymorphism:
- Primitive types: `Number` (f64), `Boolean` (i1), `String` (ptr), `Null`
- User-defined types with single inheritance
- Protocols (structural interface types)
- Vectors (`[T]`), ranges (built-in iterable)
- `HulkType::Unknown` as a placeholder during type collection
- `HulkType::Never` for error propagation (avoids cascading false errors)

The `conforms` predicate implements subtyping: `A conforms to B` if `A == B` or `A` is a direct or indirect subtype of `B`, or `A` implements protocol `B`.

### 4.3 Built-in Functions

The semantic analyzer pre-registers the following built-in functions:
- `print(x)` — prints any value; returns `Null`
- `sqrt(x: Number): Number`, `sin(x): Number`, `cos(x): Number`, `exp(x): Number`
- `rand(): Number` — uniform random in [0,1)
- `range(start, end): Range` — creates a range iterable
- `len(v): Number`, `size(v): Number` — vector/range size

Mathematical constants `PI` and `E` are pre-declared as `Number` globals.

### 4.4 Error Reporting

All semantic errors are collected into a `Vec<SemanticError>` and reported after the full pass completes, so the user sees all errors in one compilation rather than just the first one. Each `SemanticError` carries a `Span` for precise source location.

---

## 5. Code Generation

### 5.1 LLVM IR via inkwell

The backend generates LLVM Intermediate Representation (IR) using `inkwell`, the Rust-safe bindings to the LLVM 17 C API. The IR is then optimized and compiled to native machine code.

The `CodegenContext` struct holds all LLVM state: the `Context`, `Module`, `Builder`, and auxiliary maps (scope stack, type registry, type hierarchy, etc.).

### 5.2 Value Representation

HULK's type system requires different LLVM types for different values:

| HULK type | LLVM type | CgValue variant |
|-----------|-----------|-----------------|
| `Number`  | `double`  | `Number(FloatValue)` |
| `Boolean` | `i1`      | `Bool(IntValue)` |
| `String`  | `ptr`     | `Str(PointerValue)` |
| User type | `ptr` to struct | `Object(PointerValue)` |
| Vector    | `ptr` to heap buf | `Vector(PointerValue)` |
| `Null`    | `ptr` null | `Null` |

The `CgValue` enum tracks the runtime type alongside the LLVM value, enabling correct coercions (e.g., `Bool → f64` for the JIT entry return, `Number → ptr` for passing to polymorphic print).

### 5.3 Object Layout

Every user-defined type is laid out as an LLVM struct with the following fields:

```
{ i32 type_tag, ptr vtable_ptr, field0, field1, ... }
```

- **`type_tag`**: A unique integer assigned during compilation in DFS pre-order of the inheritance tree. Because subtypes receive contiguous tags after their parents, the `is` type test compiles to a range check: `tag >= parent_min_tag && tag <= parent_max_tag`.
- **`vtable_ptr`**: Points to the vtable for this object's concrete type.
- **Fields**: All fields from ancestor types come first (in ancestor order), followed by the type's own fields.

Vtables are LLVM global structs of function pointers. Each slot corresponds to a method; overriding replaces the pointer in the same slot. Method dispatch is a single load from the vtable followed by an indirect call.

### 5.4 Inheritance and Polymorphism

Method resolution during code generation:

1. **Static dispatch**: For known concrete types (non-polymorphic calls), the method function is called directly by name (`__hulk_method_<Type>_<method>`).
2. **Virtual dispatch**: For expressions of interface/base type, the vtable is loaded from the object pointer and the method slot is indexed by name.

The vtable is initialized in the global initializer for each type, populated with function pointers to the concrete implementations.

### 5.5 Optimization

The IR is optimized using the LLVM new pass manager with the pipeline:

```
mem2reg,instcombine,reassociate,simplifycfg
```

- **mem2reg**: Promotes `alloca`/`store`/`load` patterns to SSA registers, enabling subsequent optimizations.
- **instcombine**: Constant folding, algebraic simplification.
- **reassociate**: Reorders expressions for better constant folding.
- **simplifycfg**: Removes dead branches and simplifies the control flow graph.

---

## 6. Runtime Library

The runtime is a C library (`runtime/hulk_runtime.c`) compiled to `hulk_runtime.a` and linked into every HULK executable. It provides:

- **`hulk_print`**: Prints a string followed by a newline to stdout.
- **`hulk_str_from_number`**: Converts `f64` to a C string. If the number is an integer (`.fract() == 0` and within i64 range), formats without decimal point (`42` not `42.0`).
- **`hulk_str_concat`** / **`hulk_str_concat_space`**: String concatenation for `@` and `@@`.
- **`hulk_str_eq`**, **`hulk_str_size`**: String comparison and length.
- **`hulk_vec_alloc`**, **`hulk_vec_get`**, **`hulk_vec_size`**: Heap-allocated vector operations.
- **`hulk_range_alloc`**, **`hulk_range_next`**, **`hulk_range_current`**: Range iterator for `for` loops.
- **`hulk_rand`**: Uniform random number.
- **`hulk_type_error`**: Runtime type error handler.

---

## 7. AOT Compilation Pipeline

When `./hulk source.hulk` is invoked:

1. Lex, parse, and type-check the source.
2. Generate LLVM IR for the entire program.
3. Add a C-compatible `main()` that calls `__hulk_entry()` (the top-level expression wrapper) and returns 0.
4. Optimize the IR.
5. Emit a native object file to `/tmp/hulk_program.o` using `TargetMachine::write_to_file`.
6. Link with `hulk_runtime.a` using system `gcc`: `gcc /tmp/hulk_program.o hulk_runtime.a -o ./output -lm`.

The resulting `./output` is a self-contained native Linux x86-64 ELF binary with no dependency on LLVM at runtime.

---

## 8. Implemented Features

The following HULK language features are fully implemented:

- All arithmetic operators: `+`, `-`, `*`, `/`, `%`, `^` (power)
- String operators: `@` (concatenation), `@@` (concatenation with space)
- Comparison and logical operators: `==`, `!=`, `<`, `>`, `<=`, `>=`, `&&`, `||`, `!`
- `let`-`in` with multiple simultaneous bindings
- Destructive assignment `:=` for variables and object fields
- `if`-`elif`-`else` expressions
- `while` loops
- `for (x in iterable)` loops with `range(start, end)`
- Named functions with optional type annotations (full type inference for unannotated parameters and return types)
- Type declarations with constructor parameters, annotated and unannotated fields
- Method definitions with `=>` shorthand and block bodies
- `self` inside methods and initializers
- Single inheritance via `inherits`
- `base(...)` keyword for calling parent constructors
- Virtual method dispatch (polymorphism)
- `is` type test (compiled to an efficient range check on type tags)
- `as` type cast (downcast)
- Protocols with method signatures
- Protocol conformance checking
- Vector literals `[a, b, c]` and generator expressions `[expr || x in range]`
- Vector indexing `v[i]`
- Built-in functions: `print`, `sqrt`, `sin`, `cos`, `exp`, `rand`, `range`, `len`, `size`
- String, Number, Boolean literal types
- `true` and `false` literals
- Mathematical constants (available as global names)

---

## 9. Known Limitations

- **Lambdas**: Anonymous function expressions (`(x) => x * 2`) are not yet supported.
- **String escape sequences**: `\n`, `\t`, `\"` inside string literals are not processed; the backslash and following character appear literally.
- **`toString()`**: Calling `print(obj)` where `obj` is a user-defined type calls the object's `toString()` method if defined, but if no `toString()` is overridden, it prints a fallback representation rather than a formatted string.
- **Memory management**: The compiler uses a simple bump/malloc allocation strategy with no garbage collection. Long-running programs with many string or object allocations will leak memory.
- **Error recovery**: The parser stops at the first syntactic error. Multiple syntactic errors in the same file are not reported in a single pass.
- **Integer arithmetic**: HULK uses `Number` (IEEE 754 double) for all numeric values. There is no separate integer type, which means very large integers may lose precision.

---

## 10. Design Decisions and Trade-offs

**Why LALR(1) from scratch?** Using a hand-built parser generator made the grammar and table construction fully transparent and debuggable. It also eliminates a dependency on external tooling, which simplifies the build process.

**Why LLVM / inkwell?** LLVM provides a mature, well-optimized backend for x86-64. Using `inkwell` (Rust-safe LLVM bindings) avoids the dangers of raw C FFI while giving access to the full LLVM optimization pipeline.

**Why `force-dynamic` LLVM linking?** Static linking against LLVM produces executables of 50–200 MB. Dynamic linking against `libLLVM-17.so` keeps the compiler binary small and shares the LLVM library with other tools on the system.

**Why a runtime C library?** The runtime handles operations that are difficult to implement purely in LLVM IR: formatted printing, string allocation, heap-allocated vectors, and the range iterator. Separating these into a C library (`hulk_runtime.a`) makes them easy to inspect, test, and replace.

**Why DFS pre-order type tags for `is`?** The `is` operator is very common in HULK OOP code. Assigning type tags in DFS pre-order of the inheritance tree guarantees that all subtypes of a given type have contiguous tags, turning the `is` check into a single range comparison — O(1) at runtime without a vtable lookup or class object traversal.

---

## 11. Testing

The compiler includes a test suite (`cargo test`) with 384 tests covering:
- Lexer: tokenization of all token types, edge cases, error tokens
- Parser: parsing of all grammar productions, AST shape verification
- Semantic analysis: type checking, error detection, protocol conformance
- Code generation and JIT execution: arithmetic, control flow, functions, OOP, vectors, ranges, built-ins

All 384 tests pass on the reference development platform (Ubuntu 22.04 with LLVM 17).
