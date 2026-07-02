//! SlugArch CXL host runtime.

pub mod dispatch;
pub mod error;
pub mod host;
pub mod job;
pub mod qemu_type2;
pub mod replay;
pub mod result;
pub mod sim_feasible;

pub use error::HostError;
pub use host::CxlHost;
pub use job::{GemmJob, GemmResult};
pub use replay::{
    CxlDirection, CxlEndpoint, CxlRecordMode, CxlRecordPolicy, CxlRecordedRun, CxlReplayArtifact,
    CxlReplayRecord, CxlReplaySummary, CxlReplayValidation, CxlTransactionClass,
};
pub use sim_feasible::{
    measure_replay_metadata, PayloadRecordCounts, ReplayMetadataReport, ReplayModeMeasurement,
};
