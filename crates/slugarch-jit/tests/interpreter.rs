use slugarch_jit::{
    AddressRange, Decision, Direction, Engine, EpochPolicy, Event, EventClass, JitErrorCode,
    PayloadCapture, Policy, RecordMode, Rule, MAX_EVENT_PAYLOAD, SLUG_JIT_ABI_VERSION,
};

fn policy(mode: RecordMode, epoch_policy: EpochPolicy, rules: Vec<Rule>) -> Policy {
    Policy {
        version: SLUG_JIT_ABI_VERSION,
        name: "interpreter-test".to_string(),
        allowed_classes: vec![
            EventClass::CxlMemRead,
            EventClass::CxlMemWrite,
            EventClass::CxlMemData,
            EventClass::Completion,
            EventClass::Phase,
            EventClass::Fence,
        ],
        ranges: vec![AddressRange {
            base: 80 * 1024 * 1024,
            length: 32 * 1024 * 1024,
        }],
        sample_stride: 1,
        record_mode: mode,
        metadata_budget: 256,
        epoch_policy,
        rules,
    }
}

fn emitting_policy(mode: RecordMode) -> Policy {
    policy(
        mode,
        EpochPolicy::Phase,
        vec![
            Rule::Capture { mode },
            Rule::Emit,
            Rule::EpochFromPhase,
            Rule::Halt,
        ],
    )
}

fn event_with_payload(payload: &[u8]) -> Event {
    let mut bytes = [0; MAX_EVENT_PAYLOAD];
    bytes[..payload.len()].copy_from_slice(payload);
    Event {
        event_id: 1,
        client_id: 7,
        direction: Direction::HostToDevice,
        class: EventClass::CxlMemWrite,
        opcode: 0x44,
        address: 80 * 1024 * 1024,
        payload_len: payload.len() as u8,
        payload: bytes,
        tag: 11,
        phase_id: 3,
        monotonic_ns: 900,
        status: 0,
    }
}

fn emitted(decision: Decision) -> slugarch_jit::ReplayRecord {
    match decision {
        Decision::Emit { record } => record,
        other => panic!("expected emitted record, got {other:?}"),
    }
}

#[test]
fn validation_record_is_deterministic() {
    let verified = emitting_policy(RecordMode::Validation).verify().unwrap();
    let mut left = Engine::new(verified.clone());
    let mut right = Engine::new(verified);
    let event = event_with_payload(&[1, 0, 2]);

    assert_eq!(
        left.observe(&event).unwrap(),
        right.observe(&event).unwrap()
    );
    assert_eq!(left.stats().record_count, 1);
    assert_eq!(left.stats().event_count, 1);
}

#[test]
fn validation_delta_and_full_capture_are_exact() {
    let event = event_with_payload(&[1, 0, 2]);

    let mut validation = Engine::new(emitting_policy(RecordMode::Validation).verify().unwrap());
    let record = emitted(validation.observe(&event).unwrap());
    assert_eq!(
        record.payload,
        PayloadCapture::Validation {
            length: 3,
            hash: 0xd0a3991867273472,
        }
    );

    let mut delta = Engine::new(emitting_policy(RecordMode::Delta).verify().unwrap());
    let record = emitted(delta.observe(&event).unwrap());
    let PayloadCapture::Delta {
        length,
        pair_count,
        pairs,
    } = record.payload
    else {
        panic!("expected delta capture");
    };
    assert_eq!(length, 3);
    assert_eq!(pair_count, 2);
    assert_eq!((pairs[0].index, pairs[0].value), (0, 1));
    assert_eq!((pairs[1].index, pairs[1].value), (2, 2));
    assert!(pairs[2..].iter().all(|pair| pair.value == 0));

    let mut full = Engine::new(emitting_policy(RecordMode::Full).verify().unwrap());
    let record = emitted(full.observe(&event).unwrap());
    let PayloadCapture::Full { length, bytes } = record.payload else {
        panic!("expected full capture");
    };
    assert_eq!(length, 3);
    assert_eq!(&bytes[..3], &[1, 0, 2]);
    assert!(bytes[3..].iter().all(|byte| *byte == 0));
}

#[test]
fn strict_reject_does_not_emit() {
    let rejecting = policy(
        RecordMode::Validation,
        EpochPolicy::Phase,
        vec![
            Rule::Capture {
                mode: RecordMode::Validation,
            },
            Rule::Reject { code: 7 },
            Rule::EpochFromPhase,
            Rule::Halt,
        ],
    );
    let mut engine = Engine::new(rejecting.verify().unwrap());
    let decision = engine.observe(&event_with_payload(&[1, 0, 2])).unwrap();

    assert!(matches!(decision, Decision::Reject { code: 7 }));
    assert_eq!(engine.stats().record_count, 0);
    assert_eq!(engine.stats().reject_count, 1);
}

#[test]
fn phase_and_increment_epochs_are_applied_before_record_commit() {
    let event = event_with_payload(&[1]);
    let mut phase = Engine::new(emitting_policy(RecordMode::Validation).verify().unwrap());
    assert_eq!(emitted(phase.observe(&event).unwrap()).epoch, 3);

    let increment = policy(
        RecordMode::Validation,
        EpochPolicy::Increment,
        vec![
            Rule::Capture {
                mode: RecordMode::Validation,
            },
            Rule::Emit,
            Rule::EpochIncrement,
            Rule::Halt,
        ],
    );
    let mut engine = Engine::new(increment.verify().unwrap());
    assert_eq!(emitted(engine.observe(&event).unwrap()).epoch, 1);
    assert_eq!(emitted(engine.observe(&event).unwrap()).epoch, 2);
}

#[test]
fn sampling_and_range_edges_choose_one_forward_path() {
    let mut sampled = policy(
        RecordMode::Validation,
        EpochPolicy::Phase,
        vec![
            Rule::Sample { stride: 2, skip: 3 },
            Rule::Capture {
                mode: RecordMode::Validation,
            },
            Rule::Emit,
            Rule::EpochFromPhase,
            Rule::Halt,
        ],
    );
    sampled.sample_stride = 2;
    let mut engine = Engine::new(sampled.verify().unwrap());
    let first = event_with_payload(&[1]);
    let mut second = first.clone();
    second.event_id = 2;
    assert_eq!(engine.observe(&first).unwrap(), Decision::Accept);
    assert!(matches!(
        engine.observe(&second).unwrap(),
        Decision::Emit { .. }
    ));
    assert_eq!(engine.stats().event_count, 2);
    assert_eq!(engine.stats().record_count, 1);

    let ranged = policy(
        RecordMode::Validation,
        EpochPolicy::Phase,
        vec![
            Rule::MatchRange { range: 0, skip: 3 },
            Rule::Capture {
                mode: RecordMode::Validation,
            },
            Rule::Emit,
            Rule::EpochFromPhase,
            Rule::Halt,
        ],
    );
    let mut engine = Engine::new(ranged.verify().unwrap());
    let mut inside = event_with_payload(&[1]);
    inside.address = 112 * 1024 * 1024 - 1;
    let mut outside = inside.clone();
    outside.event_id = 2;
    outside.address += 1;
    assert!(matches!(
        engine.observe(&inside).unwrap(),
        Decision::Emit { .. }
    ));
    assert_eq!(engine.observe(&outside).unwrap(), Decision::Accept);
}

#[test]
fn unsupported_or_malformed_event_poison_without_stats_mutation() {
    let verified = emitting_policy(RecordMode::Validation).verify().unwrap();
    let mut engine = Engine::new(verified);
    let mut unsupported = event_with_payload(&[]);
    unsupported.class = EventClass::KernelLaunch;
    assert_eq!(
        engine.observe(&unsupported).unwrap_err().code(),
        JitErrorCode::Unsupported
    );
    assert_eq!(engine.stats(), Default::default());

    let valid = event_with_payload(&[1]);
    assert_eq!(
        engine.observe(&valid).unwrap_err().code(),
        JitErrorCode::Poisoned
    );
    assert_eq!(engine.stats(), Default::default());

    let mut engine = Engine::new(emitting_policy(RecordMode::Validation).verify().unwrap());
    let mut malformed = event_with_payload(&[1]);
    malformed.payload[4] = 1;
    assert_eq!(
        engine.observe(&malformed).unwrap_err().code(),
        JitErrorCode::Unsupported
    );
    assert_eq!(engine.stats(), Default::default());
}

#[test]
fn repeated_events_have_monotonic_sequences_and_stats() {
    let mut engine = Engine::new(emitting_policy(RecordMode::Full).verify().unwrap());
    for event_id in 1..=64 {
        let mut event = event_with_payload(&[1, 0, 2]);
        event.event_id = event_id;
        let record = emitted(engine.observe(&event).unwrap());
        assert_eq!(record.sequence, event_id);
        assert_eq!(engine.stats().event_count, event_id);
        assert_eq!(engine.stats().record_count, event_id);
    }
    assert_eq!(engine.stats().drop_count, 0);
}
