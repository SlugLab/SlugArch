//! Sanity: our frontend parses + lowers real PTX kernels. Uses the vendored
//! gemm.ptx fixture.

use slugarch_ir::module::Context;
use slugarch_ir::op::Op;
use std::fs;

#[test]
fn gemm_fixture_parses_and_lowers() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/gemm.ptx");
    let text = fs::read_to_string(path).expect("read gemm.ptx");

    let parsed = slugarch_ptx_frontend::parse_ptx(&text).expect("parse");
    let mut ctx = Context::new();
    let m = slugarch_ptx_frontend::lower_to_slugir(&parsed, &mut ctx).expect("lower");

    assert_eq!(m.functions.len(), 1, "gemm.ptx has one .entry");
    let f = &m.functions[0];
    assert_eq!(f.name, "gemm");

    let mut counts: std::collections::HashMap<&'static str, usize> =
        std::collections::HashMap::new();
    for id in &f.order {
        let tag = match f.ops.get(id).unwrap() {
            Op::Arith { .. } => "arith",
            Op::TensorTile { .. } => "tensor_tile",
            Op::StateStep { .. } => "state_step",
            Op::Dma { .. } => "dma",
            Op::Emu { .. } => "emu",
        };
        *counts.entry(tag).or_default() += 1;
    }
    assert!(
        counts.get("arith").copied().unwrap_or(0) >= 10,
        "expected >=10 Arith ops, got {}",
        counts.get("arith").copied().unwrap_or(0)
    );
    assert!(
        counts.get("dma").copied().unwrap_or(0) >= 5,
        "expected >=5 Dma ops, got {}",
        counts.get("dma").copied().unwrap_or(0)
    );
    assert!(
        f.order.len() >= 60 && f.order.len() <= 120,
        "gemm.ptx op count drift: {}",
        f.order.len()
    );
}
