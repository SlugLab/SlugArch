use slugarch_ir::types::{IpId, OpId, TokenId};

#[derive(Debug, thiserror::Error)]
pub enum FabricError {
    #[error("dispatch timeout on {ip:?} after {cycles} cycles")]
    DispatchTimeout {
        ip: IpId,
        cycles: u64,
        cmd_op: Option<OpId>,
    },

    #[error("token resolution stuck; {pending} commands could not proceed")]
    TokenResolutionStuck { pending: usize },

    #[error("determinism mismatch at cycle {actual} (expected {expected})")]
    DeterminismMismatch { expected: u64, actual: u64 },

    #[error("value mismatch in region {region}: expected hash {expected}, got {actual}")]
    ValueMismatch {
        region: String,
        expected: String,
        actual: String,
    },

    #[error("binding failed: {0}")]
    Bind(#[from] slugarch_backend::BindError),

    #[error("ir pass failed: {0}")]
    Ir(#[from] slugarch_ir::IrError),

    #[error("frontend failed: {0}")]
    Frontend(#[from] slugarch_ptx_frontend::FrontendError),

    #[error("i/o: {0}")]
    Io(String),

    #[error("serialization: {0}")]
    Serialize(String),

    #[error("unknown token dependency: {0:?}")]
    UnknownToken(TokenId),
}
