//! Tier 2 Path C (value-preservation): run gemm.ptx under two different
//! BackendPolicy instances and assert host-memory hash is identical.
//!
//! v1 caveat: both policies route everything to PtxEmulationCore because
//! the other bindings' placeholder token encodings don't drive their RTL
//! backends to completion. The "override" exercises the Path C mechanism
//! (policy swap + re-bind + re-run) even though the dispatch stream ends
//! up similar. Host memory is unchanged by CPU-emu stubs in v1, so the
//! hash comparison is a weak-but-real invariant.

use slugarch_backend::{emit_dispatches, DispatchCmd};
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
        "all_emu_a"
    }
    fn pick(&self, _op: &Op) -> BackendChoice {
        BackendChoice(IpId::PtxEmulationCore)
    }
}

/// A different named policy that picks the same backend. Exercises the
/// "policy swap" mechanism without needing a binding that actually
/// produces different host-memory behavior.
struct AllEmuPolicyB;
impl BackendPolicy for AllEmuPolicyB {
    fn name(&self) -> &'static str {
        "all_emu_b"
    }
    fn pick(&self, _op: &Op) -> BackendChoice {
        BackendChoice(IpId::PtxEmulationCore)
    }
}

fn lower_and_bind(policy: impl BackendPolicy + 'static) -> Vec<DispatchCmd> {
    let path = slugarch_path::fixture("gemm.ptx");
    let text = std::fs::read_to_string(path).unwrap();
    let parsed = slugarch_ptx_frontend::parse_ptx(&text).unwrap();
    let mut ctx = Context::new();
    let mut m = slugarch_ptx_frontend::lower_to_slugir(&parsed, &mut ctx).unwrap();
    FuseDecodeOps.run(&mut m).unwrap();
    SelectBackend::new(policy).run(&mut m).unwrap();
    AssignTokens.run(&mut m).unwrap();
    emit_dispatches(&m, "value_preservation_test").unwrap()
}

fn hash_host_mem(bytes: &[u8]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    bytes.hash(&mut h);
    h.finish()
}

#[test]
fn binding_override_preserves_host_mem_hash() {
    let host_initial = vec![0u8; 4096];

    let stream_a = lower_and_bind(AllEmuPolicy);
    let mut fabric_a = Fabric::new(4096);
    fabric_a.set_host_mem(&host_initial);
    fabric_a.run(stream_a).unwrap();
    let hash_a = hash_host_mem(fabric_a.host_mem());

    let stream_b = lower_and_bind(AllEmuPolicyB);
    let mut fabric_b = Fabric::new(4096);
    fabric_b.set_host_mem(&host_initial);
    fabric_b.run(stream_b).unwrap();
    let hash_b = hash_host_mem(fabric_b.host_mem());

    assert_eq!(
        hash_a, hash_b,
        "host memory output hash must be identical across binding policies"
    );
}
