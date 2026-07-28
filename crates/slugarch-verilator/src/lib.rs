//! Safe Rust wrapper over slugarch-verilator-sys.

pub mod hj;
pub mod impls;
pub mod wire;

pub use hj::{HjError, HjObservation, HjRecordImage, HjStats, VerilatedHj};
pub use impls::VerilatedIp;
pub use wire::{WireCmd, WireDone, TOKEN_BYTES};
