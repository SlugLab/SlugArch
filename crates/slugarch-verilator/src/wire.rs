/// Token wire — 256 bits, byte-addressable, little-endian.
pub const TOKEN_BYTES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireCmd {
    pub valid: bool,
    pub token_in: [u8; TOKEN_BYTES],
}

impl WireCmd {
    pub fn invalid() -> Self {
        Self {
            valid: false,
            token_in: [0; TOKEN_BYTES],
        }
    }
    pub fn new(token_in: [u8; TOKEN_BYTES]) -> Self {
        Self {
            valid: true,
            token_in,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireDone {
    pub valid: bool,
    pub token_out: [u8; TOKEN_BYTES],
}
