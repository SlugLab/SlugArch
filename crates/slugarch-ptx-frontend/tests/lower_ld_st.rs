use slugarch_ir::module::Context;
use slugarch_ir::op::Op;

const KERNEL: &str = r#"
.version 7.0
.target sm_70
.address_size 64

.visible .entry ld_st() {
    .reg .b64 %rd<3>;
    .reg .b32 %r<2>;
    ld.global.u32 %r1, [%rd1];
    st.global.u32 [%rd2], %r1;
    ret;
}
"#;

#[test]
fn ld_and_st_lower_to_dma() {
    let parsed = slugarch_ptx_frontend::parse_ptx(KERNEL).expect("parse");
    let mut ctx = Context::new();
    let m = slugarch_ptx_frontend::lower_to_slugir(&parsed, &mut ctx).expect("lower");
    let f = &m.functions[0];
    let dmas: Vec<_> = f.order.iter().filter(|id| matches!(f.ops.get(id), Some(Op::Dma { .. }))).collect();
    assert_eq!(dmas.len(), 2, "expected one Dma per ld/st");
}
