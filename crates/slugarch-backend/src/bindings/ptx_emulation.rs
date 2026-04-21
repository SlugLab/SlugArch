use crate::{BackendBinding, BindCtx, BindError, DispatchCmd, DispatchMeta};
use slugarch_ir::op::Op;
use slugarch_ir::types::IpId;

/// CPU-backed emulation binding. Unlike the other bindings this does not
/// dispatch to Verilator RTL — Plan 3's fabric engine runs these on the
/// CPU against the host memory buffer.
pub struct PtxEmulationBinding;

impl BackendBinding for PtxEmulationBinding {
    fn ip(&self) -> IpId {
        IpId::PtxEmulationCore
    }

    fn bind(&self, op: &Op, ctx: &BindCtx) -> Result<Vec<DispatchCmd>, BindError> {
        match op {
            Op::Emu { opcode, operands } => {
                let mut token = [0u8; 32];
                let n = operands.len() as u32;
                token[..4].copy_from_slice(&n.to_le_bytes());
                Ok(vec![DispatchCmd {
                    ip: IpId::PtxEmulationCore,
                    opcode: *opcode,
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
                choice: IpId::PtxEmulationCore,
                op_desc: format!("{:?}", op),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slugarch_ir::op::OperandRef;
    use slugarch_ir::types::TokenId;

    #[test]
    fn emu_passes_opcode_through() {
        let op = Op::Emu {
            opcode: 17,
            operands: vec![OperandRef::ImmU64(0)],
        };
        let ctx = BindCtx {
            token_in: TokenId(0),
            token_out: TokenId(1),
            source_hint: None,
            policy: None,
        };
        let cmds = PtxEmulationBinding.bind(&op, &ctx).unwrap();
        assert_eq!(cmds[0].opcode, 17);
        assert_eq!(cmds[0].ip, IpId::PtxEmulationCore);
    }
}
