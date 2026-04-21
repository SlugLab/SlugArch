//! SlugArch fabric — drives VerilatedIp models + CPU-backed ptx_emulation
//! from a DispatchCmd stream, with record/replay support.

pub mod cpu_emu;
pub mod engine;
pub mod error;
pub mod record;

pub use engine::{Fabric, RunReport};
pub use error::FabricError;
pub use record::ReplayArtifact;
