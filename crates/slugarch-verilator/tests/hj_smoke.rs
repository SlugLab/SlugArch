use slugarch_jit::{
    AddressRange, Decision, Direction, Engine, EpochPolicy, Event, EventClass, PayloadCapture,
    Policy, RecordMode, Rule, MAX_EVENT_PAYLOAD, SLUG_JIT_ABI_VERSION,
};
use slugarch_verilator::{HjError, VerilatedHj};

fn policy(
    mode: RecordMode,
    epoch_policy: EpochPolicy,
    sample_stride: u32,
    rules: Vec<Rule>,
) -> Policy {
    Policy {
        version: SLUG_JIT_ABI_VERSION,
        name: "verilator-hj-smoke".to_string(),
        allowed_classes: vec![
            EventClass::CxlMemRead,
            EventClass::CxlMemWrite,
            EventClass::CxlMemData,
            EventClass::Completion,
        ],
        ranges: vec![AddressRange {
            base: 80 * 1024 * 1024,
            length: 32 * 1024 * 1024,
        }],
        sample_stride,
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
        1,
        vec![
            Rule::Capture { mode },
            Rule::Emit,
            Rule::EpochFromPhase,
            Rule::Halt,
        ],
    )
}

fn validation_policy() -> Policy {
    emitting_policy(RecordMode::Validation)
}

fn rejecting_policy() -> Policy {
    policy(
        RecordMode::Validation,
        EpochPolicy::Phase,
        1,
        vec![
            Rule::Capture {
                mode: RecordMode::Validation,
            },
            Rule::Reject { code: 7 },
            Rule::EpochFromPhase,
            Rule::Halt,
        ],
    )
}

fn sampling_policy() -> Policy {
    policy(
        RecordMode::Validation,
        EpochPolicy::Phase,
        2,
        vec![
            Rule::Sample { stride: 2, skip: 3 },
            Rule::Capture {
                mode: RecordMode::Validation,
            },
            Rule::Emit,
            Rule::EpochFromPhase,
            Rule::Halt,
        ],
    )
}

fn cxl_write_event_with_payload(event_id: u64, bytes: &[u8]) -> Event {
    let mut payload = [0; MAX_EVENT_PAYLOAD];
    payload[..bytes.len()].copy_from_slice(bytes);
    Event {
        event_id,
        client_id: 0,
        direction: Direction::HostToDevice,
        class: EventClass::CxlMemWrite,
        opcode: 0,
        address: 80 * 1024 * 1024,
        payload_len: bytes.len() as u8,
        payload,
        tag: 7,
        phase_id: 3,
        monotonic_ns: 0,
        status: 0,
    }
}

fn cxl_write_event() -> Event {
    cxl_write_event_with_payload(1, &[1, 2, 3, 4, 5, 6, 7, 8])
}

#[test]
fn load_policy_and_observe_one_write() {
    let verified = validation_policy().verify().unwrap();
    let mut hj = VerilatedHj::new().unwrap();
    hj.reset().unwrap();
    hj.load_policy(&verified).unwrap();
    let result = hj.observe(&cxl_write_event()).unwrap();
    let record = result.record.expect("validation policy must emit");

    assert_eq!(record.event_id, 1);
    assert_eq!(record.epoch, 3);
    assert_eq!(result.stats.event_count, 1);
    assert_eq!(result.stats.record_count, 1);
    assert_eq!(result.stats.metadata_bytes, 8);
    assert_eq!(result.stats.drop_count, 0);
    assert!(result.cycles > 0);
}

#[test]
fn repeated_events_do_not_create_phantom_endpoint_observations() {
    let verified = validation_policy().verify().unwrap();
    let mut hj = VerilatedHj::new().unwrap();
    hj.reset().unwrap();
    hj.load_policy(&verified).unwrap();

    for event_id in 1..=8 {
        let event = cxl_write_event_with_payload(event_id, &[event_id as u8]);
        let result = hj.observe(&event).unwrap();
        let record = result.record.expect("every input must emit once");
        assert_eq!(record.sequence, event_id);
        assert_eq!(record.event_id, event_id);
        assert_eq!(result.stats.event_count, event_id);
        assert_eq!(result.stats.record_count, event_id);
        assert_eq!(result.stats.metadata_bytes, event_id * 8);
        assert_eq!(result.stats.drop_count, 0);
    }
}

#[test]
fn reject_and_sample_false_are_non_error_terminal_decisions() {
    let mut hj = VerilatedHj::new().unwrap();
    hj.reset().unwrap();
    hj.load_policy(&rejecting_policy().verify().unwrap())
        .unwrap();
    let rejected = hj.observe(&cxl_write_event()).unwrap();
    assert!(!rejected.accepted);
    assert_eq!(rejected.reject_code, Some(7));
    assert!(rejected.record.is_none());
    assert_eq!(rejected.stats.event_count, 1);
    assert_eq!(rejected.stats.reject_count, 1);
    assert_eq!(rejected.stats.record_count, 0);
    assert_eq!(rejected.stats.policy_error, 0);

    hj.reset().unwrap();
    hj.load_policy(&sampling_policy().verify().unwrap())
        .unwrap();
    let unsampled = hj
        .observe(&cxl_write_event_with_payload(41, &[1, 0, 2]))
        .unwrap();
    assert!(unsampled.accepted);
    assert_eq!(unsampled.reject_code, None);
    assert!(unsampled.record.is_none());
    assert_eq!(unsampled.stats.event_count, 1);
    assert_eq!(unsampled.stats.record_count, 0);
    assert_eq!(unsampled.stats.policy_error, 0);

    let sampled = hj
        .observe(&cxl_write_event_with_payload(42, &[1, 0, 2]))
        .unwrap();
    assert!(sampled.accepted);
    assert_eq!(sampled.record.unwrap().sequence, 1);
    assert_eq!(sampled.stats.event_count, 2);
    assert_eq!(sampled.stats.record_count, 1);
}

#[test]
fn delta_and_full_records_match_the_software_interpreter() {
    for (mode, payload) in [
        (RecordMode::Delta, vec![1, 0, 2]),
        (RecordMode::Full, (1_u8..=32).collect()),
    ] {
        let verified = emitting_policy(mode).verify().unwrap();
        let event = cxl_write_event_with_payload(1, &payload);
        let mut software = Engine::new(verified.clone());
        let Decision::Emit { record: expected } = software.observe(&event).unwrap() else {
            panic!("software interpreter did not emit");
        };

        let mut hj = VerilatedHj::new().unwrap();
        hj.reset().unwrap();
        hj.load_policy(&verified).unwrap();
        let observed = hj.observe(&event).unwrap();
        assert_eq!(observed.record, Some(expected));
        assert_eq!(
            observed.stats.metadata_bytes,
            software.stats().metadata_bytes
        );
    }
}

#[test]
fn dense_delta_is_rejected_before_rtl_state_changes() {
    let verified = emitting_policy(RecordMode::Delta).verify().unwrap();
    let payload = [1_u8; 17];
    let mut hj = VerilatedHj::new().unwrap();
    hj.load_policy(&verified).unwrap();
    let baseline = hj.stats().unwrap();

    assert!(matches!(
        hj.observe(&cxl_write_event_with_payload(1, &payload)),
        Err(HjError::Unsupported(
            "delta payload exceeds 16 nonzero pairs"
        ))
    ));
    assert_eq!(hj.stats().unwrap(), baseline);
}

#[test]
fn unrepresentable_events_are_rejected_before_rtl_state_changes() {
    let verified = validation_policy().verify().unwrap();
    let mut hj = VerilatedHj::new().unwrap();
    hj.reset().unwrap();
    hj.load_policy(&verified).unwrap();
    let baseline = hj.stats().unwrap();

    let mut cases = Vec::new();
    let mut long_payload = cxl_write_event();
    long_payload.payload_len = 33;
    cases.push(long_payload);
    let mut wide_opcode = cxl_write_event();
    wide_opcode.opcode = 0x10;
    cases.push(wide_opcode);
    let mut wide_tag = cxl_write_event();
    wide_tag.tag = u64::from(u16::MAX) + 1;
    cases.push(wide_tag);
    let mut unsupported_class = cxl_write_event();
    unsupported_class.class = EventClass::Phase;
    cases.push(unsupported_class);

    for event in cases {
        assert!(matches!(hj.observe(&event), Err(HjError::Unsupported(_))));
        assert_eq!(hj.stats().unwrap(), baseline);
    }
}

#[test]
fn unrepresentable_policy_and_reset_preserve_explicit_boundaries() {
    let subset = Policy {
        allowed_classes: vec![EventClass::CxlMemWrite],
        ..validation_policy()
    }
    .verify()
    .unwrap();
    let mut hj = VerilatedHj::new().unwrap();
    hj.reset().unwrap();
    assert!(matches!(hj.load_policy(&subset), Err(HjError::Policy(_))));
    assert!(matches!(
        hj.observe(&cxl_write_event()),
        Err(HjError::Protocol("no policy is loaded"))
    ));

    hj.load_policy(&validation_policy().verify().unwrap())
        .unwrap();
    hj.observe(&cxl_write_event()).unwrap();
    hj.reset().unwrap();
    let stats = hj.stats().unwrap();
    assert_eq!(stats.event_count, 0);
    assert_eq!(stats.record_count, 0);
    assert_eq!(stats.metadata_bytes, 0);
    assert_eq!(stats.policy_error, 0);
    assert!(!stats.policy_ready);
}

#[test]
fn full_record_exposes_exact_payload_bytes() {
    let verified = emitting_policy(RecordMode::Full).verify().unwrap();
    let payload: Vec<u8> = (1_u8..=32).collect();
    let mut hj = VerilatedHj::new().unwrap();
    hj.reset().unwrap();
    hj.load_policy(&verified).unwrap();
    let record = hj
        .observe(&cxl_write_event_with_payload(1, &payload))
        .unwrap()
        .record
        .unwrap();
    let PayloadCapture::Full { length, bytes } = record.payload else {
        panic!("expected full capture");
    };
    assert_eq!(length, 32);
    assert_eq!(&bytes[..32], payload);
    assert!(bytes[32..].iter().all(|byte| *byte == 0));
}
