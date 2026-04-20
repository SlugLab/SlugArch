//! SlugArch intermediate representation.

pub mod types;
pub mod op;
pub mod graph;
pub mod module;
pub mod error;
pub mod pass;
pub mod serialize;
pub mod passes;

pub use error::IrError;
pub use module::{Context, Function, Module};
pub use op::{ArithKind, Op, StateKind, TileKind};
pub use types::{Addr, BackendChoice, Dtype, IpId, OpId, Shape, TokenId};
