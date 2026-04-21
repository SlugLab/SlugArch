use crate::{BackendBinding, BindCtx, BindError, DispatchCmd, DispatchMeta};
use slugarch_ir::op::{Op, TileKind};
use slugarch_ir::types::IpId;

/// Shared binding used by all 3 systolic array sizes + gemm_ip.
/// Encoding: opcode=1 (gemm tile); token[0..4] = shape product as u32 LE.
pub struct SystolicBinding(pub IpId);

impl BackendBinding for SystolicBinding {
    fn ip(&self) -> IpId {
        self.0
    }

    fn bind(&self, op: &Op, ctx: &BindCtx) -> Result<Vec<DispatchCmd>, BindError> {
        match op {
            Op::TensorTile {
                kind: TileKind::Gemm,
                shape,
                ..
            } => {
                let mut token = [0u8; 32];
                let prod = shape.0.iter().copied().product::<u32>();
                token[..4].copy_from_slice(&prod.to_le_bytes());
                Ok(vec![DispatchCmd {
                    ip: self.0,
                    opcode: 1,
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
                choice: self.0,
                op_desc: format!("{:?}", op),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slugarch_ir::op::{Op, TileKind};
    use slugarch_ir::types::{Dtype, Shape, TokenId};

    #[test]
    fn systolic_gemm_emits_one_cmd_with_shape_product() {
        let b = SystolicBinding(IpId::SystolicArray16x16);
        let op = Op::TensorTile {
            kind: TileKind::Gemm,
            shape: Shape(vec![16, 16]),
            dtype: Dtype::F16,
            operands: vec![],
        };
        let ctx = BindCtx {
            token_in: TokenId(1),
            token_out: TokenId(2),
            source_hint: None,
            policy: None,
        };
        let cmds = b.bind(&op, &ctx).unwrap();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].ip, IpId::SystolicArray16x16);
        assert_eq!(cmds[0].opcode, 1);
        let prod = u32::from_le_bytes(cmds[0].token[..4].try_into().unwrap());
        assert_eq!(prod, 256);
    }
}
