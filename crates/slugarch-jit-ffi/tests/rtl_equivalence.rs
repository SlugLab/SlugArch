#![cfg(feature = "fpga-verilator")]

use std::mem::size_of;
use std::ptr::null_mut;

use slugarch_jit::{
    AddressRange, Decision, Direction, Engine, EpochPolicy, Event, EventClass, PayloadCapture,
    Policy, RecordMode, ReplayRecord, Rule, Stats, MAX_EVENT_PAYLOAD, SLUG_JIT_ABI_VERSION,
};
use slugarch_jit_ffi::fpga::FpgaBackend;
use slugarch_jit_ffi::{
    slugarch_jit_backend_caps, slugarch_jit_create, slugarch_jit_destroy, slugarch_jit_load_policy,
    slugarch_jit_observe, slugarch_jit_stats, SlugJitCreateArgs, SlugJitDecision, SlugJitEvent,
    SlugJitHandle, SlugJitPolicyInfo, SlugJitStats, SLUG_JIT_BACKEND_FPGA_VERILATOR,
    SLUG_JIT_CAP_FPGA_RTL, SLUG_JIT_ERR_UNSUPPORTED, SLUG_JIT_OK,
};

const RECORD_BYTES: usize = 128;

fn policy(
    mode: RecordMode,
    epoch_policy: EpochPolicy,
    sample_stride: u32,
    rules: Vec<Rule>,
) -> Policy {
    Policy {
        version: SLUG_JIT_ABI_VERSION,
        name: format!("rtl-equivalence-{mode:?}"),
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

fn event(event_id: u64, payload: &[u8]) -> Event {
    let mut bytes = [0; MAX_EVENT_PAYLOAD];
    bytes[..payload.len()].copy_from_slice(payload);
    Event {
        event_id,
        client_id: 9,
        direction: Direction::HostToDevice,
        class: EventClass::CxlMemWrite,
        opcode: 3,
        address: 80 * 1024 * 1024,
        payload_len: payload.len() as u8,
        payload: bytes,
        tag: 11,
        phase_id: 7,
        monotonic_ns: 900,
        status: 0,
    }
}

fn encode_record(record: &ReplayRecord) -> ([u8; RECORD_BYTES], usize) {
    let mut bytes = [0; RECORD_BYTES];
    bytes[0..3].copy_from_slice(&[1, 1, 1]);
    bytes[4..12].copy_from_slice(&record.sequence.to_le_bytes());
    bytes[12..20].copy_from_slice(&record.event_id.to_le_bytes());
    bytes[20..52].copy_from_slice(&record.policy_digest);
    bytes[52..60].copy_from_slice(&record.epoch.to_le_bytes());
    bytes[60] = record.direction as u8;
    bytes[61] = record.class as u8;
    bytes[62..64].copy_from_slice(&record.opcode.to_le_bytes());
    bytes[64..72].copy_from_slice(&record.address.to_le_bytes());
    bytes[72..80].copy_from_slice(&record.tag.to_le_bytes());
    bytes[80..84].copy_from_slice(&record.status.to_le_bytes());

    let capture_len = match &record.payload {
        PayloadCapture::Validation { length, hash } => {
            bytes[84] = RecordMode::Validation as u8;
            bytes[85] = *length;
            bytes[86] = 8;
            bytes[96..104].copy_from_slice(&hash.to_le_bytes());
            8
        }
        PayloadCapture::Delta {
            length,
            pair_count,
            pairs,
        } => {
            bytes[84] = RecordMode::Delta as u8;
            bytes[85] = *length;
            bytes[86] = pair_count * 2;
            bytes[87] = *pair_count;
            for (slot, pair) in pairs.iter().take(usize::from(*pair_count)).enumerate() {
                bytes[96 + slot * 2] = pair.index;
                bytes[97 + slot * 2] = pair.value;
            }
            usize::from(*pair_count) * 2
        }
        PayloadCapture::Full {
            length,
            bytes: data,
        } => {
            bytes[84] = RecordMode::Full as u8;
            bytes[85] = *length;
            bytes[86] = *length;
            bytes[96..96 + usize::from(*length)].copy_from_slice(&data[..usize::from(*length)]);
            usize::from(*length)
        }
    };
    (bytes, 96 + capture_len)
}

fn assert_exact(policy: Policy, events: Vec<Event>) {
    let verified = policy.verify().unwrap();
    let mut oracle = Engine::new(verified.clone());
    let mut rtl = FpgaBackend::new().unwrap();
    rtl.load_policy(&verified).unwrap();
    assert!(rtl.last_policy_load_cycles() > 0);

    for event in events {
        let expected = oracle.observe(&event).unwrap();
        let expected_image = match &expected {
            Decision::Emit { record } => Some(encode_record(record)),
            Decision::Accept | Decision::Reject { .. } => None,
        };
        let observed = rtl.observe(&event).unwrap();
        assert!(rtl.last_observation_cycles() > 0);
        assert_eq!(observed, expected);
        assert_eq!(rtl.stats(), oracle.stats());

        match (rtl.last_record_image(), expected_image) {
            (Some(actual), Some((bytes, length))) => {
                assert_eq!(actual.length, length);
                assert_eq!(actual.bytes, bytes);
            }
            (None, None) => {}
            other => panic!("record-image mismatch: {other:?}"),
        }
    }
}

#[test]
fn validation_delta_and_full_are_byte_exact_for_local_v1() {
    assert_exact(emitting_policy(RecordMode::Validation), {
        let mut boundary = event(4, &[1, 0, 3]);
        boundary.direction = Direction::DeviceToHost;
        boundary.class = EventClass::Completion;
        boundary.opcode = 15;
        boundary.address += 32 * 1024 * 1024 - 1;
        boundary.tag = u64::from(u16::MAX);
        boundary.phase_id = u64::MAX;
        boundary.status = u32::MAX;
        vec![
            event(1, &[]),
            event(2, &[1, 0, 2]),
            event(3, &(1_u8..=32).collect::<Vec<_>>()),
            boundary,
        ]
    });
    assert_exact(
        emitting_policy(RecordMode::Delta),
        vec![event(1, &[]), event(2, &[1, 0, 2]), event(3, &[1; 16])],
    );
    assert_exact(
        emitting_policy(RecordMode::Full),
        vec![
            event(1, &[]),
            event(2, &[1, 0, 2]),
            event(3, &(1_u8..=32).collect::<Vec<_>>()),
        ],
    );
}

#[test]
fn matches_sampling_reject_and_epochs_are_semantically_exact() {
    let matching_policy = |rule| {
        policy(
            RecordMode::Validation,
            EpochPolicy::Phase,
            1,
            vec![
                rule,
                Rule::Capture {
                    mode: RecordMode::Validation,
                },
                Rule::Emit,
                Rule::EpochFromPhase,
                Rule::Halt,
            ],
        )
    };

    let write = event(1, &[1]);
    let mut read = event(2, &[2]);
    read.class = EventClass::CxlMemRead;
    assert_exact(
        matching_policy(Rule::MatchClass {
            class: EventClass::CxlMemWrite,
            skip: 3,
        }),
        vec![read, write.clone()],
    );

    let mut d2h = event(1, &[1]);
    d2h.direction = Direction::DeviceToHost;
    assert_exact(
        matching_policy(Rule::MatchDirection {
            direction: Direction::HostToDevice,
            skip: 3,
        }),
        vec![d2h, write.clone()],
    );

    let mut failed = event(1, &[1]);
    failed.status = 7;
    assert_exact(
        matching_policy(Rule::MatchStatus { status: 7, skip: 3 }),
        vec![write.clone(), failed],
    );

    let ranged = policy(
        RecordMode::Validation,
        EpochPolicy::Phase,
        1,
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
    let mut below = event(1, &[1]);
    below.address -= 1;
    let mut base = event(2, &[2]);
    let mut end = event(3, &[3]);
    end.address += 32 * 1024 * 1024 - 1;
    let mut beyond = event(4, &[4]);
    beyond.address += 32 * 1024 * 1024;
    assert_exact(ranged, vec![below, base.clone(), end, beyond]);

    let sampled = policy(
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
    );
    assert_exact(sampled, vec![event(1, &[1]), event(2, &[2])]);

    let rejected = policy(
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
    );
    assert_exact(rejected, vec![event(1, &[1])]);

    let incremented = policy(
        RecordMode::Validation,
        EpochPolicy::Increment,
        1,
        vec![
            Rule::Capture {
                mode: RecordMode::Validation,
            },
            Rule::Emit,
            Rule::EpochIncrement,
            Rule::Halt,
        ],
    );
    base.phase_id = 99;
    assert_exact(incremented, vec![base.clone(), base]);
}

#[test]
fn local_v1_representability_errors_do_not_fallback_or_mutate_stats() {
    let verified = emitting_policy(RecordMode::Validation).verify().unwrap();
    let mut rtl = FpgaBackend::new().unwrap();
    rtl.load_policy(&verified).unwrap();
    let baseline = rtl.stats();

    let mut cases = Vec::new();
    let mut payload = event(1, &[0; 33]);
    payload.payload_len = 33;
    cases.push(payload);
    let mut opcode = event(2, &[]);
    opcode.opcode = 16;
    cases.push(opcode);
    let mut tag = event(3, &[]);
    tag.tag = u64::from(u16::MAX) + 1;
    cases.push(tag);
    let mut class = event(4, &[]);
    class.class = EventClass::Phase;
    cases.push(class);

    for event in cases {
        assert_eq!(
            rtl.observe(&event).unwrap_err().code() as i32,
            SLUG_JIT_ERR_UNSUPPORTED
        );
        assert_eq!(rtl.stats(), baseline);
    }
}

#[test]
fn first_supported_invalid_event_classification_matches_the_oracle() {
    let verified = emitting_policy(RecordMode::Validation).verify().unwrap();
    let mut malformed = event(1, &[1]);
    malformed.payload[1] = 1;
    let mut unsupported_class = event(2, &[]);
    unsupported_class.class = EventClass::Phase;

    for event in [malformed, unsupported_class] {
        let mut oracle = Engine::new(verified.clone());
        let mut rtl = FpgaBackend::new().unwrap();
        rtl.load_policy(&verified).unwrap();
        let expected = oracle.observe(&event).unwrap_err();
        let observed = rtl.observe(&event).unwrap_err();
        assert_eq!(observed.code(), expected.code());
        assert_eq!(rtl.stats(), oracle.stats());
        assert_eq!(rtl.last_observation_cycles(), 0);
    }
}

#[test]
fn failed_policy_load_unloads_the_previous_fpga_policy() {
    let mut rtl = FpgaBackend::new().unwrap();
    rtl.load_policy(&emitting_policy(RecordMode::Validation).verify().unwrap())
        .unwrap();
    assert!(matches!(
        rtl.observe(&event(1, &[1])).unwrap(),
        Decision::Emit { .. }
    ));

    let unsupported = Policy {
        allowed_classes: vec![EventClass::CxlMemWrite],
        ..emitting_policy(RecordMode::Validation)
    }
    .verify()
    .unwrap();
    assert_eq!(
        rtl.load_policy(&unsupported).unwrap_err().code(),
        slugarch_jit::JitErrorCode::Unsupported
    );
    assert_eq!(rtl.stats(), Stats::default());
    assert_eq!(
        rtl.observe(&event(2, &[2])).unwrap_err().code(),
        slugarch_jit::JitErrorCode::Backend
    );
}

fn ffi_event(source: &Event) -> SlugJitEvent {
    SlugJitEvent {
        struct_size: size_of::<SlugJitEvent>() as u32,
        abi_version: SLUG_JIT_ABI_VERSION,
        event_id: source.event_id,
        client_id: source.client_id,
        direction: source.direction as u32,
        event_class: source.class as u32,
        opcode: u32::from(source.opcode),
        payload_len: u32::from(source.payload_len),
        address: source.address,
        tag: source.tag,
        phase_id: source.phase_id,
        monotonic_ns: source.monotonic_ns,
        status: source.status,
        reserved: 0,
        payload: source.payload,
    }
}

#[test]
fn ffi_selects_fpga_explicitly_and_exposes_its_stats() {
    assert_ne!(slugarch_jit_backend_caps() & SLUG_JIT_CAP_FPGA_RTL, 0);
    let args = SlugJitCreateArgs {
        struct_size: size_of::<SlugJitCreateArgs>() as u32,
        abi_version: SLUG_JIT_ABI_VERSION,
        backend: SLUG_JIT_BACKEND_FPGA_VERILATOR,
        strict: 1,
        diagnostic_capacity: 256,
        reserved: 0,
    };
    let mut handle: *mut SlugJitHandle = null_mut();
    let policy = emitting_policy(RecordMode::Validation);
    let json = serde_json::to_vec(&policy).unwrap();
    let mut info = SlugJitPolicyInfo {
        struct_size: size_of::<SlugJitPolicyInfo>() as u32,
        abi_version: SLUG_JIT_ABI_VERSION,
        backend: 0,
        canonical_bytes: 0,
        digest: [0; 32],
        instruction_count: 0,
        range_count: 0,
        metadata_budget: 0,
        reserved: 0,
    };
    let mut decision = SlugJitDecision {
        struct_size: size_of::<SlugJitDecision>() as u32,
        abi_version: SLUG_JIT_ABI_VERSION,
        accepted: 0,
        emitted: 0,
        error_code: 0,
        record_bytes: 0,
        payload_bytes: 0,
        reserved: 0,
        epoch: 0,
        record_id: 0,
    };
    let mut stats = SlugJitStats {
        struct_size: size_of::<SlugJitStats>() as u32,
        abi_version: SLUG_JIT_ABI_VERSION,
        event_count: 0,
        record_count: 0,
        metadata_bytes: 0,
        reject_count: 0,
        drop_count: 0,
        epoch: 0,
    };
    let event = ffi_event(&event(1, &[1, 0, 2]));

    // SAFETY: all ABI pointers remain live for each call, and the returned
    // handle is destroyed exactly once.
    unsafe {
        assert_eq!(slugarch_jit_create(&args, &mut handle), SLUG_JIT_OK);
        assert!(!handle.is_null());
        assert_eq!(
            slugarch_jit_load_policy(handle, json.as_ptr(), json.len() as u32, &mut info),
            SLUG_JIT_OK
        );
        assert_eq!(info.backend, SLUG_JIT_BACKEND_FPGA_VERILATOR);
        assert_eq!(
            slugarch_jit_observe(handle, &event, &mut decision),
            SLUG_JIT_OK
        );
        assert_eq!((decision.accepted, decision.emitted), (1, 1));
        assert_eq!(slugarch_jit_stats(handle, &mut stats), SLUG_JIT_OK);
        assert_eq!(stats.event_count, 1);
        assert_eq!(stats.record_count, 1);
        assert_eq!(stats.metadata_bytes, 8);
        slugarch_jit_destroy(handle);
    }
}

#[test]
fn canonical_stats_type_remains_the_ffi_backend_contract() {
    let _: Stats = FpgaBackend::new().unwrap().stats();
}
