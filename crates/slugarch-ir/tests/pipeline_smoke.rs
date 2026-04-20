//! End-to-end check that all four Plan-1 passes compose on a synthetic IR module.

use slugarch_ir::graph::Edge;
use slugarch_ir::module::{Context, FunctionBuilder, Module};
use slugarch_ir::op::{Op, StateKind, TileKind};
use slugarch_ir::pass::Pass;
use slugarch_ir::passes::{AssignTokens, FuseDecodeOps, SelectBackend};
use slugarch_ir::types::{Dtype, Shape};

#[test]
fn four_passes_run_end_to_end_on_synthetic_module() {
    // Build a tiny graph: RmsNorm -> Gemm(64x64) -> AttnDecode.
    let mut ctx = Context::new();
    let mut b = FunctionBuilder::new(&mut ctx, "tiny");
    let rms = b.add_op(Op::StateStep { kind: StateKind::RmsNorm, operands: vec![] });
    let gemm = b.add_op(Op::TensorTile {
        kind: TileKind::Gemm,
        shape: Shape(vec![64, 64]),
        dtype: Dtype::F16,
        operands: vec![],
    });
    let attn = b.add_op(Op::StateStep { kind: StateKind::AttnDecode, operands: vec![] });
    b.add_edge(Edge::Data(rms, gemm));
    b.add_edge(Edge::Data(gemm, attn));
    let mut m = Module::default();
    m.functions.push(b.finish());

    FuseDecodeOps.run(&mut m).unwrap();
    SelectBackend::default_policy().run(&mut m).unwrap();
    AssignTokens.run(&mut m).unwrap();

    let f = &m.functions[0];
    // Every op now has backend + token assignments.
    for id in &f.order {
        let meta = f.meta.get(id).unwrap();
        assert!(meta.backend.is_some(), "op {id:?} missing backend");
        assert!(meta.token_in.is_some() && meta.token_out.is_some(), "op {id:?} missing tokens");
    }
    // Gemm must have landed on a systolic IP (64 -> 16x16 per DefaultPolicy).
    let gemm_ip = f.meta[&gemm].backend.unwrap().0;
    assert_eq!(gemm_ip, slugarch_ir::IpId::SystolicArray16x16);
}
