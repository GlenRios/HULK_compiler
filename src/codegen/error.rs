use thiserror::Error;

pub type CodegenResult<T> = Result<T, CodegenError>;

#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("nodo no soportado aun: {0}")]
    Unsupported(String),

    #[error("variable no definida: {0}")]
    UnknownVariable(String),

    #[error("funcion no definida: {0}")]
    UnknownFunction(String),

    #[error("lvalue invalido para asignacion")]
    InvalidLValue,

    #[error("error al parsear numero: {0}")]
    ParseNumber(String),

    #[error("error del builder LLVM: {0}")]
    Builder(String),

    #[error("modulo LLVM invalido: {0}")]
    Verify(String),

    #[error("error de JIT: {0}")]
    Jit(String),
}
