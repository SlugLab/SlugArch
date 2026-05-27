//! Tier 2 Path A: gemm.ptx end-to-end through the full SlugArch pipeline.

use slugarch_backend::emit_dispatches;
use slugarch_fabric::Fabric;
use slugarch_ir::module::Context;
use slugarch_ir::op::Op;
use slugarch_ir::pass::Pass;
use slugarch_ir::passes::select_backend::BackendPolicy;
use slugarch_ir::passes::{AssignTokens, FuseDecodeOps, SelectBackend};
use slugarch_ir::types::{BackendChoice, IpId};

/// Test-only policy: routes everything to PtxEmulationCore.
/// v1's NoC Verilator model doesn't retire our placeholder Dma token
/// encoding (token layout isn't derived from port_bindings yet), so
/// the Tier 2 E2E test runs the whole kernel on CPU emu until real
/// encodings land (post-v1).
struct AllEmuPolicy;
impl BackendPolicy for AllEmuPolicy {
    fn name(&self) -> &'static str {
        "all_emu_v1"
    }
    fn pick(&self, _op: &Op) -> BackendChoice {
        BackendChoice(IpId::PtxEmulationCore)
    }
}

fn lower_gemm() -> slugarch_ir::module::Module {
    let path = slugarch_path::fixture("gemm.ptx");
    let text = std::fs::read_to_string(path).expect("read gemm.ptx");
    let parsed = slugarch_ptx_frontend::parse_ptx(&text).expect("parse");
    let mut ctx = Context::new();
    let mut m = slugarch_ptx_frontend::lower_to_slugir(&parsed, &mut ctx).expect("lower");
    FuseDecodeOps.run(&mut m).unwrap();
    SelectBackend::new(AllEmuPolicy).run(&mut m).unwrap();
    AssignTokens.run(&mut m).unwrap();
    m
}

#[test]
fn gemm_runs_end_to_end() {
    let m = lower_gemm();
    let stream = emit_dispatches(&m, "default_v1").expect("bind");
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
