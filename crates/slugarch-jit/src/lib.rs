pub mod error;
pub mod event;
pub mod policy;
pub mod program;
pub mod verifier;

pub use error::{JitError, JitErrorCode};
pub use event::{Direction, Event, EventClass, MAX_EVENT_PAYLOAD};
pub use policy::{AddressRange, EpochPolicy, Policy, RecordMode, Rule};
pub use program::{Instruction, VerifiedPolicy, MAX_INSTRUCTIONS, MAX_RANGES};

pub const SLUG_JIT_ABI_VERSION: u32 = 1;
pub const SLUG_JIT_EVENT_VERSION: u32 = 1;
pub const SLUG_JIT_PACKET_VERSION: u32 = 1;
pub const SLUG_JIT_BACKEND_CONTRACT_VERSION: u32 = 1;
