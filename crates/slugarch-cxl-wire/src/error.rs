use crate::msg::MsgClass;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WireError {
    #[error("unknown message class {0:#x}")]
    UnknownClass(u8),
    #[error("unknown opcode {0:#x} for class {1:?}")]
    UnknownOpcode(u8, MsgClass),
    #[error("reserved bits set in flit header")]
    ReservedBitsSet,
    #[error("tag mismatch: expected {expected:#x}, got {got:#x}")]
    TagMismatch { expected: u16, got: u16 },
}
