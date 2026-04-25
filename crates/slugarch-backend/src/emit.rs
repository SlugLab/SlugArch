//! Walk a SlugIR Module and bind every op to its assigned IP.
//!
//! `select_backend` annotates each op with `meta.backend`; this helper turns
//! that annotation into a flat `Vec<DispatchCmd>` by routing through the
//! per-IP `BackendBinding` for the chosen IP. Callers used to inline this
//! loop (CLI + 3 fabric tests, all near-identical).

use crate::bindings::{
    GemmIpBinding, NoCMeshBinding, NpuClusterBinding, NpuSeedGBinding, PtxEmulationBinding,
    SystolicBinding,
};
use crate::{BackendBinding, BindCtx, BindError, DispatchCmd};
use slugarch_ir::module::Module;
use slugarch_ir::op::Op;
use slugarch_ir::types::{IpId, TokenId};

/// Bind every op in `m` to a `DispatchCmd` per `meta.backend`. `policy_name`
/// is recorded in `DispatchMeta::policy` for downstream observability.
///
/// Non-Emu ops routed to PtxEmulationCore use opcode 253 (the v1 catchall
/// for emu-on-arith dispatches).
pub fn emit_dispatches(m: &Module, policy_name: &str) -> Result<Vec<DispatchCmd>, BindError> {
    let mut out: Vec<DispatchCmd> = Vec::new();
    for f in &m.functions {
        for id in &f.order {
            let op = f.ops.get(id).expect("order references missing op");
            let meta = f.meta.get(id).expect("order references missing meta");
            let ip = meta
                .backend
                .expect("emit_dispatches requires SelectBackend to have run")
                .0;
            let ctx = BindCtx {
                token_in: meta.token_in.unwrap_or(TokenId(0)),
                token_out: meta.token_out.unwrap_or(TokenId(0)),
                source_hint: meta.source_hint.as_deref(),
                policy: Some(policy_name),
            };
            let cmds = match ip {
                IpId::PtxEmulationCore => {
                    let opcode = match op {
                        Op::Emu { opcode, .. } => *opcode,
                        _ => 253,
                    };
                    PtxEmulationBinding.bind(
                        &Op::Emu {
                            opcode,
                            operands: vec![],
                        },
                        &ctx,
                    )
                }
                IpId::NoCMesh => NoCMeshBinding.bind(op, &ctx),
                IpId::SystolicArray4x4 => SystolicBinding(IpId::SystolicArray4x4).bind(op, &ctx),
                IpId::SystolicArray16x16 => {
                    SystolicBinding(IpId::SystolicArray16x16).bind(op, &ctx)
                }
                IpId::SystolicArray32x32 => {
                    SystolicBinding(IpId::SystolicArray32x32).bind(op, &ctx)
                }
                IpId::NpuArrayV4SeedG => NpuSeedGBinding.bind(op, &ctx),
                IpId::NpuClusterV4 => NpuClusterBinding.bind(op, &ctx),
                IpId::GemmIp => GemmIpBinding.bind(op, &ctx),
                IpId::SlugCxl4x4 => {
                    return Err(BindError::NoBindingForChoice {
                        choice: ip,
                        op_desc: format!("{:?}", op),
                    });
                }
            }?;
            out.extend(cmds);
        }
    }
    Ok(out)
}
