use proptest::prelude::*;
use slugarch_jit::{
    AddressRange, Direction, Engine, EpochPolicy, Event, EventClass, Policy, RecordMode, Rule,
    MAX_EVENT_PAYLOAD, SLUG_JIT_ABI_VERSION,
};

fn verified_policy() -> slugarch_jit::VerifiedPolicy {
    Policy {
        version: SLUG_JIT_ABI_VERSION,
        name: "property-test".to_string(),
        allowed_classes: vec![EventClass::CxlMemRead, EventClass::CxlMemWrite],
        ranges: vec![AddressRange {
            base: 0,
            length: u64::MAX,
        }],
        sample_stride: 1,
        record_mode: RecordMode::Full,
        metadata_budget: 256,
        epoch_policy: EpochPolicy::Phase,
        rules: vec![
            Rule::Capture {
                mode: RecordMode::Full,
            },
            Rule::Emit,
            Rule::EpochFromPhase,
            Rule::Halt,
        ],
    }
    .verify()
    .unwrap()
}

proptest! {
    #[test]
    fn arbitrary_policy_bytes_never_panic(
        bytes in proptest::collection::vec(any::<u8>(), 0..4096)
    ) {
        let _ = Policy::parse(&bytes);
    }

    #[test]
    fn valid_events_are_bounded_and_stats_never_decrease(
        event_id in 1u64..=u64::MAX,
        phase_id in any::<u64>(),
        payload in proptest::collection::vec(any::<u8>(), 0..=MAX_EVENT_PAYLOAD),
        is_write in any::<bool>(),
    ) {
        let mut bytes = [0; MAX_EVENT_PAYLOAD];
        bytes[..payload.len()].copy_from_slice(&payload);
        let event = Event {
            event_id,
            client_id: 1,
            direction: if is_write {
                Direction::HostToDevice
            } else {
                Direction::DeviceToHost
            },
            class: if is_write {
                EventClass::CxlMemWrite
            } else {
                EventClass::CxlMemRead
            },
            opcode: 0,
            address: 0,
            payload_len: payload.len() as u8,
            payload: bytes,
            tag: event_id,
            phase_id,
            monotonic_ns: 0,
            status: 0,
        };
        let mut engine = Engine::new(verified_policy());
        let before = engine.stats();
        let decision = engine.observe(&event).unwrap();
        let after = engine.stats();
        let emitted = matches!(decision, slugarch_jit::Decision::Emit { .. });

        prop_assert!(emitted);
        prop_assert!(after.event_count >= before.event_count);
        prop_assert!(after.record_count >= before.record_count);
        prop_assert_eq!(after.event_count, 1);
        prop_assert_eq!(after.record_count, 1);
        prop_assert_eq!(after.drop_count, 0);
    }
}
