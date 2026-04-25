//! Tier 2 Path B (determinism): run gemm.ptx twice with identical inputs,
//! assert byte-identical RunReport.

use slugarch_backend::emit_dispatches;
use slugarch_fabric::Fabric;
use slugarch_ir::module::Context;
use slugarch_ir::op::Op;
use slugarch_ir::pass::Pass;
use slugarch_ir::passes::select_backend::BackendPolicy;
use slugarch_ir::passes::{AssignTokens, FuseDecodeOps, SelectBackend};
use slugarch_ir::types::{BackendChoice, IpId};

struct AllEmuPolicy;
impl BackendPolicy for AllEmuPolicy {
    fn name(&self) -> &'static str {
        "all_emu_v1"
    }
    fn pick(&self, _op: &Op) -> BackendChoice {
        BackendChoice(IpId::PtxEmulationCore)
    }
}

fn lower() -> slugarch_ir::module::Module {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/gemm.ptx");
    let text = std::fs::read_to_string(path).unwrap();
    let parsed = slugarch_ptx_frontend::parse_ptx(&text).unwrap();
    let mut ctx = Context::new();
    let mut m = slugarch_ptx_frontend::lower_to_slugir(&parsed, &mut ctx).unwrap();
    FuseDecodeOps.run(&mut m).unwrap();
    SelectBackend::new(AllEmuPolicy).run(&mut m).unwrap();
    AssignTokens.run(&mut m).unwrap();
    m
}

#[test]
fn same_binding_replay_is_cycle_identical() {
    let host_initial = vec![0u8; 4096];

    let m1 = lower();
    let s1 = emit_dispatches(&m1, "determinism_test").unwrap();
    let mut f1 = Fabric::new(4096);
    f1.set_host_mem(&host_initial);
    let r1 = f1.run(s1).unwrap();

    let m2 = lower();
    let s2 = emit_dispatches(&m2, "determinism_test").unwrap();
    let mut f2 = Fabric::new(4096);
    f2.set_host_mem(&host_initial);
    let r2 = f2.run(s2).unwrap();

    assert_eq!(r1, r2, "RunReport must be byte-identical across runs");
    assert_eq!(f1.host_mem(), f2.host_mem(), "host memory must match");
}
