use slugarch_ir::module::Context;
use slugarch_ir::op::Op;

const KERNEL: &str = r#"
.version 7.0
.target sm_70
.address_size 64

.visible .entry emu_ops() {
    .reg .s32 %r<8>;
    .reg .f32 %f<4>;
    and.b32 %r1, %r2, %r3;
    xor.b32 %r4, %r2, %r3;
    sqrt.approx.f32 %f1, %f2;
    ret;
}
"#;

#[test]
fn bit_ops_and_transcendentals_lower_to_emu_with_expected_opcodes() {
    let parsed = slugarch_ptx_frontend::parse_ptx(KERNEL).expect("parse ok");
    let mut ctx = Context::new();
    let m = slugarch_ptx_frontend::lower_to_slugir(&parsed, &mut ctx).expect("lower ok");
    let f = &m.functions[0];
    let opcodes: Vec<u32> = f.order.iter().filter_map(|id| match f.ops.get(id).unwrap() {
        Op::Emu { opcode, .. } => Some(*opcode),
        _ => None,
    }).collect();
    assert!(opcodes.contains(&2),  "and => opcode 2");
    assert!(opcodes.contains(&4),  "xor => opcode 4");
    assert!(opcodes.contains(&17), "sqrt => opcode 17");
    // `ret` is now recognized as control flow -> Emu 254.
    assert!(opcodes.contains(&254));
}
