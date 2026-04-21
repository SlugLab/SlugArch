//! SlugArch intermediate representation.

pub mod error;
pub mod graph;
pub mod module;
pub mod op;
pub mod pass;
pub mod passes;
pub mod serialize;
pub mod types;

pub use error::IrError;
pub use module::{Context, Function, Module};
pub use op::{ArithKind, Op, OpMeta, OperandRef, StateKind, TileKind};
pub use types::{Addr, BackendChoice, Dtype, IpId, OpId, Shape, TokenId};
