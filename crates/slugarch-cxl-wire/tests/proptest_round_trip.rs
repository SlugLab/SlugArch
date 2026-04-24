use proptest::prelude::*;
use slugarch_cxl_wire::{decode, encode, CxlMsg, M2SReqOp, M2SRwDOp, S2MDRSOp, S2MNDROp};

prop_compose! {
    fn arb_data()(bytes in prop::array::uniform32(any::<u8>())) -> [u8; 32] {
        bytes
    }
}

fn arb_msg() -> impl Strategy<Value = CxlMsg> {
    prop_oneof![
        (
            any::<u16>(),
            prop_oneof![
                Just(M2SReqOp::MemRd),
                Just(M2SReqOp::MemRdData),
                Just(M2SReqOp::MemInv),
            ],
            any::<u64>()
        )
            .prop_map(|(tag, opcode, addr)| CxlMsg::M2SReq { tag, opcode, addr }),
        (
            any::<u16>(),
            prop_oneof![
                Just(M2SRwDOp::MemWr),
                Just(M2SRwDOp::MemWrPtl),
                Just(M2SRwDOp::MemClnEvct),
            ],
            any::<u64>(),
            arb_data()
        )
            .prop_map(|(tag, opcode, addr, data)| CxlMsg::M2SRwD {
                tag,
                opcode,
                addr,
                data
            }),
        (
            any::<u16>(),
            prop_oneof![Just(S2MDRSOp::MemData), Just(S2MDRSOp::MemDataNxm),],
            arb_data()
        )
            .prop_map(|(tag, opcode, data)| CxlMsg::S2MDRS { tag, opcode, data }),
        (
            any::<u16>(),
            prop_oneof![
                Just(S2MNDROp::Cmp),
                Just(S2MNDROp::CmpS),
                Just(S2MNDROp::CmpE),
                Just(S2MNDROp::CmpI),
                Just(S2MNDROp::MemPassDirty),
                Just(S2MNDROp::DispatchFailed),
            ]
        )
            .prop_map(|(tag, opcode)| CxlMsg::S2MNDR { tag, opcode }),
    ]
}

proptest! {
    #[test]
    fn round_trip(msg in arb_msg()) {
        let back = decode(&encode(&msg)).unwrap();
        prop_assert_eq!(back, msg);
    }
}
