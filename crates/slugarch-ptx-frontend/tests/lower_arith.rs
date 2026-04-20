use slugarch_ir::module::Context;
use slugarch_ir::op::{ArithKind, Op};

const ADD_KERNEL: &str = r#"
.version 7.0
.target sm_70
.address_size 64

.visible .entry add_i32() {
    .reg .s32 %r<3>;
    add.s32 %r1, %r2, 1;
    ret;
}
"#;

#[test]
fn add_instruction_lowers_to_op_arith_add() {
    let parsed = slugarch_ptx_frontend::parse_ptx(ADD_KERNEL).expect("parse ok");
    let mut ctx = Context::new();
    let m = slugarch_ptx_frontend::lower_to_slugir(&parsed, &mut ctx).expect("lower ok");
    let ops: Vec<_> = m.functions[0].order.iter()
        .map(|id| m.functions[0].ops.get(id).unwrap())
        .collect();
    // Expect 2 ops: Arith(Add), and the Emu(255) for `ret`.
    assert_eq!(ops.len(), 2);
    assert!(matches!(ops[0], Op::Arith { kind: ArithKind::Add, .. }));
    assert!(matches!(ops[1], Op::Emu { opcode: 254, .. }));
}
