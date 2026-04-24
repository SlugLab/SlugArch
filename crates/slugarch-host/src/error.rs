use slugarch_cxl_wire::WireError;

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("dispatch failed at tag {tag:#x}: {reason}")]
    DispatchFailed { tag: u16, reason: String },

    #[error("unexpected tag: got {got:#x}, outstanding {outstanding:?}")]
    UnexpectedTag { got: u16, outstanding: Vec<u16> },

    #[error("wire decode failed: {0}")]
    WireDecode(#[from] WireError),

    #[error("flit timeout after {ticks} ticks")]
    FlitTimeout { ticks: u64 },

    #[error("cycles exceeded: limit {limit}, actual {actual}")]
    CyclesExceeded { limit: u64, actual: u64 },
}
