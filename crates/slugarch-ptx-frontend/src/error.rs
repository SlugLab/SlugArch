#[derive(Debug, thiserror::Error)]
pub enum FrontendError {
    #[error("ptx_parser returned {0} error(s); first: {1}")]
    Parse(usize, String),

    #[error("unsupported PTX op '{op}' at hint '{hint}'")]
    UnsupportedOp { op: String, hint: String },

    #[error("unresolved symbol '{name}' at hint '{hint}'")]
    SymbolResolution { name: String, hint: String },

    #[error("ir construction failed: {0}")]
    Ir(#[from] slugarch_ir::IrError),
}
