use slugarch_ir::module::Context;

const SIMPLE: &str = r#"
.version 7.0
.target sm_70
.address_size 64

.visible .entry k() {
    ret;
}
"#;

#[test]
fn single_entry_produces_one_function_with_at_least_one_op() {
    let parsed = slugarch_ptx_frontend::parse_ptx(SIMPLE).expect("parse ok");
    let mut ctx = Context::new();
    let m = slugarch_ptx_frontend::lower_to_slugir(&parsed, &mut ctx).expect("lower ok");
    assert_eq!(m.functions.len(), 1);
    assert_eq!(m.functions[0].name, "k");
    // ret is unknown in v1 -> Emu 255; we expect exactly 1 op.
    assert_eq!(m.functions[0].order.len(), 1);
}
