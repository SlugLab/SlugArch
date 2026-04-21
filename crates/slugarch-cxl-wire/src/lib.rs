//! CXL wire types and FLIT encode/decode for SlugArch Plan 4.

pub mod error;
pub mod flit;
pub mod msg;

pub mod encode;
pub mod decode;

pub use error::WireError;
pub use flit::{FlitBytes, FLIT_BYTES};
pub use msg::{
    CxlMsg, MsgClass,
    M2SReqOp, M2SRwDOp, S2MDRSOp, S2MNDROp,
    D2HReqOp, D2HRespOp, H2DReqOp, H2DRespOp,
};
pub use encode::encode;
pub use decode::decode;
