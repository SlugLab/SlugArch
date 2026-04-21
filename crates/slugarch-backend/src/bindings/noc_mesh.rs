use crate::{BackendBinding, BindCtx, BindError, DispatchCmd, DispatchMeta};
use slugarch_ir::op::Op;
use slugarch_ir::types::IpId;

/// Maps Dma ops to NoC dispatches.
/// Encoding: opcode=0; token[0..8]=src, token[8..16]=dst, token[16..24]=bytes
/// (all little-endian u64).
pub struct NoCMeshBinding;

impl BackendBinding for NoCMeshBinding {
    fn ip(&self) -> IpId {
        IpId::NoCMesh
    }

    fn bind(&self, op: &Op, ctx: &BindCtx) -> Result<Vec<DispatchCmd>, BindError> {
        match op {
            Op::Dma { src, dst, bytes } => {
                let mut token = [0u8; 32];
                token[0..8].copy_from_slice(&src.to_le_bytes());
                token[8..16].copy_from_slice(&dst.to_le_bytes());
                token[16..24].copy_from_slice(&bytes.to_le_bytes());
                Ok(vec![DispatchCmd {
                    ip: IpId::NoCMesh,
                    opcode: 0,
                    token,
                    token_in: ctx.token_in,
                    token_out: ctx.token_out,
                    meta: DispatchMeta {
                        source_hint: ctx.source_hint.map(|s| s.to_string()),
                        policy: ctx.policy.map(|s| s.to_string()),
                    },
                }])
            }
            _ => Err(BindError::NoBindingForChoice {
                choice: IpId::NoCMesh,
                op_desc: format!("{:?}", op),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slugarch_ir::types::TokenId;

    #[test]
    fn dma_packs_src_dst_bytes() {
        let op = Op::Dma {
            src: 0xdead_beef,
            dst: 0xcafe_f00d,
            bytes: 256,
        };
        let ctx = BindCtx {
            token_in: TokenId(0),
            token_out: TokenId(1),
            source_hint: None,
            policy: None,
        };
        let cmds = NoCMeshBinding.bind(&op, &ctx).unwrap();
        assert_eq!(cmds[0].opcode, 0);
        let src = u64::from_le_bytes(cmds[0].token[0..8].try_into().unwrap());
        let dst = u64::from_le_bytes(cmds[0].token[8..16].try_into().unwrap());
        let bytes = u64::from_le_bytes(cmds[0].token[16..24].try_into().unwrap());
        assert_eq!(src, 0xdead_beef);
        assert_eq!(dst, 0xcafe_f00d);
        assert_eq!(bytes, 256);
    }
}
