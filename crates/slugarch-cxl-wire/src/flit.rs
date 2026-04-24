//! FLIT byte layout.
//!
//! v1 layout (documented, not CXL 2.0/3.0 spec-compliant):
//!
//!   byte 0     : class (high nibble) | opcode (low nibble)
//!   bytes 1-2  : tag (little-endian u16)
//!   bytes 3-10 : addr (little-endian u64)
//!   bytes 11-42: data (32 bytes; valid only for RwD / DRS / D2HResp)
//!   bytes 43-63: reserved; must be zero

pub const FLIT_BYTES: usize = 64;

pub type FlitBytes = [u8; FLIT_BYTES];

/// Byte offsets within a FLIT.
#[allow(dead_code)]
pub(crate) const OFFSET_CLASS_OPCODE: usize = 0;
pub(crate) const OFFSET_TAG: usize = 1;
pub(crate) const OFFSET_ADDR: usize = 3;
pub(crate) const OFFSET_DATA: usize = 11;
pub(crate) const OFFSET_RESERVED: usize = 43;

/// Data field length.
pub const DATA_BYTES: usize = 32;
