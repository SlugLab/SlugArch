use crate::bindings::NpuSeedGBinding;
use crate::{BackendBinding, BindCtx, BindError, DispatchCmd};
use slugarch_ir::op::Op;
use slugarch_ir::types::IpId;

/// Clustered variant — reuses the seed-G opcode scheme but routes to
/// NpuClusterV4. Useful when select_backend picks cluster over the
/// bare array for multi-tile state updates.
pub struct NpuClusterBinding;

impl BackendBinding for NpuClusterBinding {
    fn ip(&self) -> IpId {
        IpId::NpuClusterV4
    }

    fn bind(&self, op: &Op, ctx: &BindCtx) -> Result<Vec<DispatchCmd>, BindError> {
        let inner = NpuSeedGBinding.bind(op, ctx)?;
        Ok(inner
            .into_iter()
            .map(|mut cmd| {
                cmd.ip = IpId::NpuClusterV4;
                cmd
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slugarch_ir::op::{Op, StateKind};
    use slugarch_ir::types::TokenId;

    #[test]
    fn cluster_forwards_to_seed_g_opcode_with_cluster_ip() {
        let op = Op::StateStep {
            kind: StateKind::AttnDecode,
            operands: vec![],
        };
        let ctx = BindCtx {
            token_in: TokenId(0),
            token_out: TokenId(1),
            source_hint: None,
            policy: None,
        };
        let cmds = NpuClusterBinding.bind(&op, &ctx).unwrap();
        assert_eq!(cmds[0].ip, IpId::NpuClusterV4);
        assert_eq!(cmds[0].opcode, 2);
    }
}
