//! CXL wire types and FLIT encode/decode for SlugArch Plan 4.

pub mod error;
pub mod flit;
pub mod msg;

pub mod decode;
pub mod encode;

pub use decode::decode;
pub use encode::encode;
pub use error::WireError;
pub use flit::{FlitBytes, FLIT_BYTES};
pub use msg::{
    CxlMsg, D2HReqOp, D2HRespOp, H2DReqOp, H2DRespOp, M2SReqOp, M2SRwDOp, MsgClass, S2MDRSOp,
    S2MNDROp,
};
