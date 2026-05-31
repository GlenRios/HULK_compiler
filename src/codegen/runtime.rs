// src/codegen/runtime.rs
//
// Dos responsabilidades:
//   1. Declarar funciones externas en el módulo LLVM (declare ... en IR)
//      para que el codegen pueda emitir calls a ellas.
//   2. Implementar las funciones hulk_* en Rust con #[unsafe(no_mangle)] extern "C"
//      para que el JIT las encuentre automáticamente en el proceso en ejecución.

use std::alloc::{alloc, alloc_zeroed, Layout};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};

use inkwell::module::Linkage;
use inkwell::values::FunctionValue;

use super::context::CodegenContext;

// ─────────────────────────────────────────────────────────────────────────────
//  Declaraciones LLVM — métodos en CodegenContext
// ─────────────────────────────────────────────────────────────────────────────

impl<'ctx> CodegenContext<'ctx> {
    /// Declara una función externa en el módulo si no existe ya.
    fn declare_extern(
        &self,
        name:    &str,
        fn_type: inkwell::types::FunctionType<'ctx>,
    ) -> FunctionValue<'ctx> {
        self.module.get_function(name).unwrap_or_else(|| {
            self.module.add_function(name, fn_type, Some(Linkage::External))
        })
    }

    /// Registra todas las funciones del runtime en el módulo LLVM.
    /// Se llama una sola vez al inicio de visit_program.
    pub fn register_runtime(&mut self) {
        let f64_t  = self.f64_type();
        let ptr_t  = self.ptr_type();
        let void_t = self.context.void_type();
        let i32_t  = self.context.i32_type();
        let i64_t  = self.context.i64_type();

        // ── libm: funciones matemáticas ───────────────────────────────────
        // Todas reciben y devuelven f64, excepto pow/log que reciben 2 f64.
        let f1 = f64_t.fn_type(&[f64_t.into()], false);
        let f2 = f64_t.fn_type(&[f64_t.into(), f64_t.into()], false);

        self.declare_extern("sqrt", f1);
        self.declare_extern("sin",  f1);
        self.declare_extern("cos",  f1);
        self.declare_extern("exp",  f1);
        self.declare_extern("log",  f1);  // libm log = logaritmo natural
        self.declare_extern("pow",  f2);  // pow(base, exp)

        // ── C stdlib ──────────────────────────────────────────────────────
        self.declare_extern("malloc", ptr_t.fn_type(&[i64_t.into()], false));
        self.declare_extern("rand",   i32_t.fn_type(&[], false));

        // ── HULK runtime (implementadas abajo en este mismo archivo) ──────
        self.declare_extern("hulk_print",
            void_t.fn_type(&[ptr_t.into()], false));

        self.declare_extern("hulk_rand",
            f64_t.fn_type(&[], false));

        self.declare_extern("hulk_str_from_number",
            ptr_t.fn_type(&[f64_t.into()], false));

        self.declare_extern("hulk_str_concat",
            ptr_t.fn_type(&[ptr_t.into(), ptr_t.into()], false));

        self.declare_extern("hulk_str_concat_space",
            ptr_t.fn_type(&[ptr_t.into(), ptr_t.into()], false));

        self.declare_extern("hulk_str_size",
            f64_t.fn_type(&[ptr_t.into()], false));

        self.declare_extern("hulk_vec_alloc",
            ptr_t.fn_type(&[i32_t.into(), i32_t.into()], false));

        self.declare_extern("hulk_vec_get",
            ptr_t.fn_type(&[ptr_t.into(), i32_t.into(), i32_t.into()], false));

        // ── Vector y Range (implementadas abajo) ─────────────────────────
        let bool_t = self.context.bool_type();
        self.declare_extern("hulk_vec_size",
            f64_t.fn_type(&[ptr_t.into()], false));
        self.declare_extern("hulk_range_alloc",
            ptr_t.fn_type(&[f64_t.into(), f64_t.into()], false));
        self.declare_extern("hulk_range_next",
            bool_t.fn_type(&[ptr_t.into()], false));
        self.declare_extern("hulk_range_current",
            f64_t.fn_type(&[ptr_t.into()], false));

        // ── String equality ───────────────────────────────────────────────
        self.declare_extern("hulk_str_eq",
            bool_t.fn_type(&[ptr_t.into(), ptr_t.into()], false));
    }

    // ── Accesores de conveniencia ─────────────────────────────────────────────
    // Permiten obtener un FunctionValue sin repetir la lógica de declare.

    pub fn get_fn(&self, name: &str) -> Option<FunctionValue<'ctx>> {
        self.module.get_function(name)
    }

    pub fn require_fn(&self, name: &str) -> super::error::CodegenResult<FunctionValue<'ctx>> {
        self.module.get_function(name)
            .ok_or_else(|| super::error::CodegenError::Unsupported(
                format!("runtime fn '{name}' no declarada — llama register_runtime() primero")))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Implementaciones Rust — disponibles para el JIT en el proceso actual
//
//  #[unsafe(no_mangle)] + extern "C" hace que el símbolo tenga nombre C plano y
//  ABI C. El JIT de LLVM resuelve símbolos externos buscando en el proceso
//  en ejecución (dlsym), así que estas funciones son visibles automáticamente.
// ─────────────────────────────────────────────────────────────────────────────

/// Imprime un string null-terminated seguido de newline.
#[unsafe(no_mangle)]
pub extern "C" fn hulk_print(s: *const c_char) {
    if s.is_null() {
        println!("null");
        return;
    }
    // Safety: el compilador siempre pasa strings null-terminated válidos.
    let msg = unsafe { CStr::from_ptr(s) };
    println!("{}", msg.to_string_lossy());
}

/// Número aleatorio en [0.0, 1.0].
/// Usa rand() de libc normalizado — suficiente para uso educativo.
#[unsafe(no_mangle)]
pub extern "C" fn hulk_rand() -> f64 {
    unsafe extern "C" { fn rand() -> c_int; }
    unsafe { rand() as f64 / 2_147_483_647.0 }
}

/// Convierte un f64 a string null-terminated.
/// Si el número es entero (sin parte decimal) lo muestra sin decimales.
#[unsafe(no_mangle)]
pub extern "C" fn hulk_str_from_number(n: f64) -> *mut c_char {
    let s = if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    };
    // into_raw() transfiere la propiedad. En un compilador real habría un GC;
    // aquí se acepta el leak porque los programas de compilador son cortos.
    CString::new(s).unwrap().into_raw()
}

/// Concatena dos strings (operador @).
#[unsafe(no_mangle)]
pub extern "C" fn hulk_str_concat(a: *const c_char, b: *const c_char) -> *mut c_char {
    let sa = ptr_to_string(a);
    let sb = ptr_to_string(b);
    CString::new(sa + &sb).unwrap().into_raw()
}

/// Concatena dos strings con espacio entre ellos (operador @@).
#[unsafe(no_mangle)]
pub extern "C" fn hulk_str_concat_space(a: *const c_char, b: *const c_char) -> *mut c_char {
    let sa = ptr_to_string(a);
    let sb = ptr_to_string(b);
    CString::new(sa + " " + &sb).unwrap().into_raw()
}

/// Longitud en bytes del string (método .size()).
#[unsafe(no_mangle)]
pub extern "C" fn hulk_str_size(s: *const c_char) -> f64 {
    if s.is_null() { return 0.0; }
    unsafe { CStr::from_ptr(s) }.to_bytes().len() as f64
}

// ── Vectores ──────────────────────────────────────────────────────────────────

/// Alloca un vector: cabecera i64 + count elementos de 8 bytes.
/// Layout: [i64 count][f64/ptr elem0][f64/ptr elem1]...
#[unsafe(no_mangle)]
pub extern "C" fn hulk_vec_alloc(count: i32, _element_size: i32) -> *mut u8 {
    let n      = count.max(0) as usize;
    let layout = Layout::from_size_align(8 + n * 8, 8).unwrap();
    let ptr    = unsafe { alloc_zeroed(layout) };
    unsafe { *(ptr as *mut i64) = n as i64; }
    ptr
}

/// Devuelve puntero al elemento i: ptr + 8 + i*8.
#[unsafe(no_mangle)]
pub extern "C" fn hulk_vec_get(vec: *mut u8, index: i32, _element_size: i32) -> *mut u8 {
    unsafe { vec.add(8 + (index as usize) * 8) }
}

/// Lee el count de la cabecera y lo devuelve como f64 (para .size()).
#[unsafe(no_mangle)]
pub extern "C" fn hulk_vec_size(vec: *mut u8) -> f64 {
    unsafe { *(vec as *const i64) as f64 }
}

// ── Range ─────────────────────────────────────────────────────────────────────

/// Alloca un Range: [f64 start][f64 end][f64 current].
/// current arranca en start-1 para que el primer next() lo lleve a start.
#[unsafe(no_mangle)]
pub extern "C" fn hulk_range_alloc(start: f64, end: f64) -> *mut u8 {
    let layout = Layout::from_size_align(24, 8).unwrap();
    let ptr    = unsafe { alloc(layout) } as *mut f64;
    unsafe { *ptr = start; *ptr.add(1) = end; *ptr.add(2) = start - 1.0; }
    ptr as *mut u8
}

/// Avanza current y devuelve si current < end.
#[unsafe(no_mangle)]
pub extern "C" fn hulk_range_next(range: *mut u8) -> bool {
    let ptr = range as *mut f64;
    unsafe { *ptr.add(2) += 1.0; *ptr.add(2) < *ptr.add(1) }
}

/// Devuelve el valor actual del iterador.
#[unsafe(no_mangle)]
pub extern "C" fn hulk_range_current(range: *mut u8) -> f64 {
    unsafe { *(range as *const f64).add(2) }
}

/// Compara dos strings por contenido (operador ==).
/// La spec HULK especifica igualdad de valor para String, no identidad de puntero.
#[unsafe(no_mangle)]
pub extern "C" fn hulk_str_eq(a: *const c_char, b: *const c_char) -> bool {
    ptr_to_string(a) == ptr_to_string(b)
}

// ── Helpers internos ──────────────────────────────────────────────────────────

fn ptr_to_string(p: *const c_char) -> String {
    if p.is_null() {
        "null".to_string()
    } else {
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}
