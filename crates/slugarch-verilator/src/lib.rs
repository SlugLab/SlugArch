//! Safe Rust wrapper over slugarch-verilator-sys.

pub mod wire;
pub mod impls;

pub use impls::VerilatedIp;
pub use wire::{WireCmd, WireDone, TOKEN_BYTES};
