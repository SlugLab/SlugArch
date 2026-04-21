//! Safe Rust wrapper over slugarch-verilator-sys.

pub mod impls;
pub mod wire;

pub use impls::VerilatedIp;
pub use wire::{WireCmd, WireDone, TOKEN_BYTES};
