use proptest::prelude::*;
use slugarch_ir::module::{Context, FunctionBuilder, Module};
use slugarch_ir::op::{ArithKind, Op, OperandRef};
use slugarch_ir::serialize::{from_bincode, from_json, to_bincode, to_json};
use slugarch_ir::types::Dtype;

fn arb_arith_op() -> impl Strategy<Value = Op> {
    (
        0u64..256,
        prop_oneof![
            Just(ArithKind::Add),
            Just(ArithKind::Mul),
            Just(ArithKind::Sub)
        ],
    )
        .prop_map(|(v, kind)| Op::Arith {
            kind,
            operands: vec![OperandRef::ImmU64(v)],
            dtype: Dtype::I32,
        })
}

fn arb_module() -> impl Strategy<Value = Module> {
    prop::collection::vec(arb_arith_op(), 1..16).prop_map(|ops| {
        let mut ctx = Context::new();
        let mut b = FunctionBuilder::new(&mut ctx, "arb");
        for op in ops {
            b.add_op(op);
        }
        let mut m = Module::default();
        m.functions.push(b.finish());
        m
    })
}

proptest! {
    #[test]
    fn json_round_trip(m in arb_module()) {
        let s = to_json(&m).unwrap();
        let back = from_json(&s).unwrap();
        prop_assert_eq!(back, m);
    }

    #[test]
    fn bincode_round_trip(m in arb_module()) {
        let bytes = to_bincode(&m).unwrap();
        let back = from_bincode(&bytes).unwrap();
        prop_assert_eq!(back, m);
    }
}
