use slugarch_ir::module::Context;
use slugarch_ir::op::Op;

const KERNEL: &str = r#"
.version 7.0
.target sm_70
.address_size 64

.visible .entry control() {
    .reg .pred %p<2>;
    bar.sync 0;
    ret;
}
"#;

#[test]
fn control_flow_lowers_to_emu_254() {
    let parsed = slugarch_ptx_frontend::parse_ptx(KERNEL).expect("parse");
    let mut ctx = Context::new();
    let m = slugarch_ptx_frontend::lower_to_slugir(&parsed, &mut ctx).expect("lower");
    let f = &m.functions[0];
    let emus: Vec<u32> = f.order.iter().filter_map(|id| match f.ops.get(id).unwrap() {
        Op::Emu { opcode, .. } => Some(*opcode),
        _ => None,
    }).collect();
    assert_eq!(emus.iter().filter(|&&o| o == 254).count(), 2, "both bar and ret -> Emu 254");
    assert!(!emus.contains(&255), "no unknown-ops left in this kernel");
}
