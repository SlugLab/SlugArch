//! SlugArch CXL host runtime.

pub mod dispatch;
pub mod error;
pub mod host;
pub mod job;
pub mod result;

pub use error::HostError;
pub use host::CxlHost;
pub use job::{GemmJob, GemmResult};
