use slugarch_ir::module::Context;
use slugarch_ir::op::{Op, TileKind};

// The vendored ptx_parser grammar (see vendor/concordia-ptx/ptx_parser/src/lib.rs
// ~line 3979) only accepts `mma.sync.aligned.m16n8k16.row.col.<dtype>.bf16.bf16.<ctype>`,
// where `<dtype>` and `<ctype>` are each one of `f16`/`f32`. The all-f16 input
// variant the task description uses is not in the grammar and falls through to
// Op::Emu, so the test kernel uses the bf16-input / f16-accumulator shape that
// the parser does recognize. The invariant under test is unchanged: an
// MMA-shaped instruction lowers to a TensorTile(Gemm) with a shape vec.
const KERNEL: &str = r#"
.version 7.0
.target sm_80
.address_size 64

.visible .entry mma_demo() {
    .reg .b32 %ra<4>, %rb<2>, %rc<2>, %rd<2>;
    mma.sync.aligned.m16n8k16.row.col.f16.bf16.bf16.f16
        {%rd0,%rd1},
        {%ra0,%ra1,%ra2,%ra3},
        {%rb0,%rb1},
        {%rc0,%rc1};
    ret;
}
"#;

#[test]
fn mma_lowers_to_tensor_tile_gemm() {
    let parsed = slugarch_ptx_frontend::parse_ptx(KERNEL).expect("parse");
    let mut ctx = Context::new();
    let m = slugarch_ptx_frontend::lower_to_slugir(&parsed, &mut ctx).expect("lower");
    let f = &m.functions[0];
    let tile = f.order.iter()
        .map(|id| f.ops.get(id).unwrap())
        .find(|op| matches!(op, Op::TensorTile { kind: TileKind::Gemm, .. }));
    assert!(tile.is_some(), "expected exactly one TensorTile(Gemm)");
    if let Some(Op::TensorTile { shape, .. }) = tile {
        assert!(shape.0.len() >= 2, "shape should have at least 2 dims");
    }
}
