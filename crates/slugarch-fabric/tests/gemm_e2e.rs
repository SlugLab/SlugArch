//! Tier 2 Path A: gemm.ptx end-to-end through the full SlugArch pipeline.

use slugarch_backend::bindings::*;
use slugarch_backend::{BackendBinding, BindCtx, DispatchCmd};
use slugarch_fabric::Fabric;
use slugarch_ir::module::Context;
use slugarch_ir::op::Op;
use slugarch_ir::pass::Pass;
use slugarch_ir::passes::select_backend::BackendPolicy;
use slugarch_ir::passes::{AssignTokens, FuseDecodeOps, SelectBackend};
use slugarch_ir::types::{BackendChoice, IpId, TokenId};

/// Test-only policy: routes everything to PtxEmulationCore.
/// v1's NoC Verilator model doesn't retire our placeholder Dma token
/// encoding (token layout isn't derived from port_bindings yet), so
/// the Tier 2 E2E test runs the whole kernel on CPU emu until real
/// encodings land (post-v1).
struct AllEmuPolicy;
impl BackendPolicy for AllEmuPolicy {
    fn name(&self) -> &'static str { "all_emu_v1" }
    fn pick(&self, _op: &Op) -> BackendChoice {
        BackendChoice(IpId::PtxEmulationCore)
    }
}

fn lower_gemm() -> slugarch_ir::module::Module {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/gemm.ptx");
    let text = std::fs::read_to_string(path).expect("read gemm.ptx");
    let parsed = slugarch_ptx_frontend::parse_ptx(&text).expect("parse");
    let mut ctx = Context::new();
    let mut m = slugarch_ptx_frontend::lower_to_slugir(&parsed, &mut ctx).expect("lower");
    FuseDecodeOps.run(&mut m).unwrap();
    SelectBackend::new(AllEmuPolicy).run(&mut m).unwrap();
    AssignTokens.run(&mut m).unwrap();
    m
}

fn opcode_from_op(op: &Op) -> u32 {
    match op {
        Op::Emu { opcode, .. } => *opcode,
        _ => 253, // v1 catchall for Arith-on-emu dispatches
    }
}

fn emit_dispatches(m: &slugarch_ir::module::Module) -> Vec<DispatchCmd> {
    let mut out: Vec<DispatchCmd> = Vec::new();
    for f in &m.functions {
        for id in &f.order {
            let op = f.ops.get(id).unwrap();
            let meta = f.meta.get(id).unwrap();
            let ip = meta.backend.expect("select_backend assigns every op").0;
            let ctx = BindCtx {
                token_in: meta.token_in.unwrap_or(TokenId(0)),
                token_out: meta.token_out.unwrap_or(TokenId(0)),
                source_hint: meta.source_hint.as_deref(),
                policy: Some("default_v1"),
            };
            let cmds = match ip {
                IpId::PtxEmulationCore => PtxEmulationBinding.bind(
                    &Op::Emu {
                        opcode: opcode_from_op(op),
                        operands: vec![],
                    },
                    &ctx,
                ),
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
            }
            .expect("bind");
            out.extend(cmds);
        }
    }
    out
}

#[test]
fn gemm_runs_end_to_end() {
    let m = lower_gemm();
    let stream = emit_dispatches(&m);
    assert!(
        stream.len() >= 50,
        "expected >=50 dispatches, got {}",
        stream.len()
    );

    let mut fabric = Fabric::new(4096);
    let report = fabric.run(stream).expect("fabric run");

    assert!(report.total_cycles > 0);
    assert!(report.completions >= 50);

    let emu_cycles = report
        .per_ip_cycles
        .get(&IpId::PtxEmulationCore)
        .copied()
        .unwrap_or(0);
    assert!(emu_cycles > 0, "expected some PtxEmulationCore cycles");

    eprintln!(
        "gemm_e2e: {} cycles total, {} completions",
        report.total_cycles, report.completions
    );
}
