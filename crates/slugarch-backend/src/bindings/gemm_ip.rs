use crate::bindings::SystolicBinding;
use crate::{BackendBinding, BindCtx, BindError, DispatchCmd};
use slugarch_ir::op::Op;
use slugarch_ir::types::IpId;

/// Identical encoding to SystolicBinding, but routes to IpId::GemmIp.
/// gemm_ip shares RTL with systolic_array_16x16 per its rtlmap, so the
/// token_in layout is the same.
pub struct GemmIpBinding;

impl BackendBinding for GemmIpBinding {
    fn ip(&self) -> IpId {
        IpId::GemmIp
    }

    fn bind(&self, op: &Op, ctx: &BindCtx) -> Result<Vec<DispatchCmd>, BindError> {
        SystolicBinding(IpId::GemmIp).bind(op, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slugarch_ir::op::{Op, TileKind};
    use slugarch_ir::types::{Dtype, Shape, TokenId};

    #[test]
    fn gemm_ip_forwards_to_systolic_encoding() {
        let op = Op::TensorTile {
            kind: TileKind::Gemm,
            shape: Shape(vec![8, 8]),
            dtype: Dtype::F16,
            operands: vec![],
        };
        let ctx = BindCtx {
            token_in: TokenId(0),
            token_out: TokenId(1),
            source_hint: None,
            policy: None,
        };
        let cmds = GemmIpBinding.bind(&op, &ctx).unwrap();
        assert_eq!(cmds[0].ip, IpId::GemmIp);
        assert_eq!(cmds[0].opcode, 1);
    }
}
