//! SlugIR -> DispatchCmd binding layer.

pub mod bindings;
pub mod descriptor;

use serde::{Deserialize, Serialize};
use slugarch_ir::op::Op;
use slugarch_ir::types::{IpId, TokenId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchCmd {
    pub ip: IpId,
    pub opcode: u32,
    /// token_in payload (256-bit wire as bytes).
    #[serde(with = "serde_token")]
    pub token: [u8; 32],
    pub token_in: TokenId,
    pub token_out: TokenId,
    pub meta: DispatchMeta,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DispatchMeta {
    pub source_hint: Option<String>,
    pub policy: Option<String>,
}

pub trait BackendBinding {
    fn ip(&self) -> IpId;
    fn bind(&self, op: &Op, ctx: &BindCtx) -> Result<Vec<DispatchCmd>, BindError>;
}

pub struct BindCtx<'a> {
    pub token_in: TokenId,
    pub token_out: TokenId,
    pub source_hint: Option<&'a str>,
    pub policy: Option<&'a str>,
}

#[derive(Debug, thiserror::Error)]
pub enum BindError {
    #[error("no binding for choice {choice:?} in op {op_desc}")]
    NoBindingForChoice { choice: IpId, op_desc: String },
    #[error("shape {shape:?} unsupported by {ip:?}")]
    ShapeUnsupported { ip: IpId, shape: Vec<u32> },
    #[error("operand out of range: addr {addr:#x}")]
    OperandOutOfRange { addr: u64 },
}

mod serde_token {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    pub fn serialize<S: Serializer>(bytes: &[u8; 32], s: S) -> Result<S::Ok, S::Error> {
        bytes.as_slice().serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
        let v: Vec<u8> = Vec::deserialize(d)?;
        v.try_into()
            .map_err(|v: Vec<u8>| serde::de::Error::invalid_length(v.len(), &"32 bytes"))
    }
}
