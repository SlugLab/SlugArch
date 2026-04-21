use crate::{BackendBinding, BindCtx, BindError, DispatchCmd, DispatchMeta};
use slugarch_ir::op::{Op, StateKind};
use slugarch_ir::types::IpId;

/// Binds StateStep ops to the NPU v4 seed_g IP. Opcode encodes StateKind.
pub struct NpuSeedGBinding;

impl BackendBinding for NpuSeedGBinding {
    fn ip(&self) -> IpId {
        IpId::NpuArrayV4SeedG
    }

    fn bind(&self, op: &Op, ctx: &BindCtx) -> Result<Vec<DispatchCmd>, BindError> {
        match op {
            Op::StateStep { kind, operands } => {
                let opcode = match kind {
                    StateKind::RmsNorm => 1,
                    StateKind::AttnDecode => 2,
                    StateKind::MlpStep => 3,
                    StateKind::DecodeGeneric => 4,
                };
                let mut token = [0u8; 32];
                let n = operands.len() as u32;
                token[..4].copy_from_slice(&n.to_le_bytes());
                Ok(vec![DispatchCmd {
                    ip: IpId::NpuArrayV4SeedG,
                    opcode,
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
                choice: IpId::NpuArrayV4SeedG,
                op_desc: format!("{:?}", op),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slugarch_ir::op::StateKind;
    use slugarch_ir::types::TokenId;

    #[test]
    fn rms_norm_maps_to_opcode_1() {
        let b = NpuSeedGBinding;
        let op = Op::StateStep {
            kind: StateKind::RmsNorm,
            operands: vec![],
        };
        let ctx = BindCtx {
            token_in: TokenId(0),
            token_out: TokenId(1),
            source_hint: None,
            policy: None,
        };
        let cmds = b.bind(&op, &ctx).unwrap();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].opcode, 1);
    }
}
