#![allow(dead_code)]
#![allow(unused)]

mod codegen;
mod lexer;
mod parser;
mod semantic;

use std::path::PathBuf;
use std::process;

use lexer::lexer::Lexer;
use lexer::master_nfa::MasterNFA;
use lexer::token::TokenType;
use lexer::token_definition::TokenDefinition;
use parser::engine::{ParseError, ParserDriver};
use parser::engine::error::ParseErrorKind;
use semantic::SemanticError;

fn cache_dir() -> PathBuf {
    let dir = if let Ok(exe) = std::env::current_exe() {
        exe.parent().unwrap_or(std::path::Path::new(".")).to_path_buf()
    } else {
        PathBuf::from(".")
    };
    let cache = dir.join(".hulk_cache");
    let _ = std::fs::create_dir_all(&cache);
    cache
}

fn load_or_build_nfa(cache_path: &std::path::Path) -> MasterNFA {
    if let Ok(bytes) = std::fs::read(cache_path) {
        if let Ok(nfa) = bincode::deserialize(&bytes) {
            return nfa;
        }
    }
    let nfa = MasterNFA::from_token_definitions(
        &TokenDefinition::default_token_definitions(),
    );
    if let Ok(bytes) = bincode::serialize(&nfa) {
        let _ = std::fs::write(cache_path, bytes);
    }
    nfa
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("uso: hulk <archivo.hulk>");
        process::exit(1);
    }

    let source = match std::fs::read_to_string(&args[1]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("(0,0) LEXICAL: no se pudo leer '{}': {}", args[1], e);
            process::exit(1);
        }
    };

    let cache = cache_dir();

    // ── 1. Análisis léxico ────────────────────────────────────────────────────
    let master = load_or_build_nfa(&cache.join("nfa.bin"));
    let mut lex    = Lexer::new(&source, master);
    let all_tokens = lex.tokenize();

    let lex_errors: Vec<_> = all_tokens
        .iter()
        .filter(|t| t.token_type == TokenType::ERROR)
        .collect();

    if !lex_errors.is_empty() {
        for tok in &lex_errors {
            eprintln!(
                "({},{}) LEXICAL: carácter inesperado '{}'",
                tok.line, tok.column, tok.lexeme
            );
        }
        process::exit(1);
    }

    // ── 2. Análisis sintáctico ────────────────────────────────────────────────
    let driver  = ParserDriver::load_or_build(&cache.join("parser.bin"));
    let program = match driver.parse(all_tokens.into_iter()) {
        Ok(p)  => p,
        Err(e) => {
            eprintln!(
                "({},{}) SYNTACTIC: {}",
                e.span.line, e.span.column,
                parse_msg(&e)
            );
            process::exit(2);
        }
    };

    // ── 3. Análisis semántico ─────────────────────────────────────────────────
    let sem_output = match semantic::analyze(&program) {
        Ok(o)    => o,
        Err(errs) => {
            for e in &errs {
                let sp = e.span();
                eprintln!("({},{}) SEMANTIC: {}", sp.line, sp.column, sem_msg(e));
            }
            process::exit(3);
        }
    };

    // ── 4. Codegen + link ─────────────────────────────────────────────────────
    let obj_path = std::env::temp_dir().join("hulk_program.o");
    let bin_path = PathBuf::from("./output");
    let runtime  = find_runtime();

    if let Err(e) = codegen::jit::compile_to_binary(
        &program, sem_output, &obj_path, &bin_path, &runtime,
    ) {
        eprintln!("(0,0) SEMANTIC: error interno de compilación: {}", e);
        process::exit(3);
    }
}

// ── Helpers de formato de errores ────────────────────────────────────────────

fn parse_msg(e: &ParseError) -> String {
    match &e.kind {
        ParseErrorKind::UnexpectedToken { lexeme, expected, .. } => {
            let mut msg = format!("token inesperado '{}'", lexeme);
            if !expected.is_empty() {
                let exp: Vec<_> = expected.iter().map(|t| format!("{}", t)).collect();
                msg.push_str(&format!(", se esperaba: {}", exp.join(" | ")));
            }
            msg
        }
        ParseErrorKind::UnexpectedEof { expected } => {
            let mut msg = "fin de archivo inesperado".to_string();
            if !expected.is_empty() {
                let exp: Vec<_> = expected.iter().map(|t| format!("{}", t)).collect();
                msg.push_str(&format!(", se esperaba: {}", exp.join(" | ")));
            }
            msg
        }
        ParseErrorKind::InternalError(msg) => {
            format!("error interno del parser: {}", msg)
        }
    }
}

fn sem_msg(e: &SemanticError) -> String {
    // El Display de SemanticError produce "[line:col] mensaje".
    // Extraemos solo la parte del mensaje.
    let full = format!("{}", e);
    if let Some(pos) = full.find("] ") {
        full[pos + 2..].to_string()
    } else {
        full
    }
}

// ── Localizar hulk_runtime.a ──────────────────────────────────────────────────
//
// El compilador busca hulk_runtime.a en orden:
//   1. Junto al propio binario (./hulk → ./hulk_runtime.a)
//   2. En el directorio de trabajo actual
//
// El Makefile lo deja siempre en la raíz del repo, que es donde el CI
// ejecuta ./hulk <archivo.hulk>.

fn find_runtime() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("hulk_runtime.a");
            if p.exists() {
                return p;
            }
        }
    }
    PathBuf::from("hulk_runtime.a")
}
