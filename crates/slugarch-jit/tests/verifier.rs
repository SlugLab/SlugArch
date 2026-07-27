use slugarch_jit::{
    AddressRange, EpochPolicy, EventClass, JitErrorCode, Policy, RecordMode, Rule,
    SLUG_JIT_ABI_VERSION,
};

fn valid_policy() -> Policy {
    Policy {
        version: SLUG_JIT_ABI_VERSION,
        name: "validation-cxlmem".to_string(),
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
        sample_stride: 1,
        record_mode: RecordMode::Validation,
        metadata_budget: 256,
        epoch_policy: EpochPolicy::Phase,
        rules: vec![
            Rule::Capture {
                mode: RecordMode::Validation,
            },
            Rule::Emit,
            Rule::EpochFromPhase,
            Rule::Halt,
        ],
    }
}

fn assert_code(policy: Policy, code: JitErrorCode) {
    assert_eq!(policy.verify().unwrap_err().code(), code);
}

#[test]
fn rejects_oversized_program_and_range_set() {
    let mut policy = valid_policy();
    policy.rules = (0..33).map(|_| Rule::Emit).collect();
    assert_code(policy, JitErrorCode::TooManyInstructions);

    let mut policy = valid_policy();
    policy.ranges = vec![
        AddressRange {
            base: 0,
            length: 64,
        };
        5
    ];
    assert_code(policy, JitErrorCode::TooManyRanges);
}

#[test]
fn rejects_wrapping_or_empty_range() {
    let mut policy = valid_policy();
    policy.ranges[0] = AddressRange {
        base: u64::MAX - 3,
        length: 8,
    };
    assert_code(policy, JitErrorCode::InvalidRange);

    let mut policy = valid_policy();
    policy.ranges[0].length = 0;
    assert_code(policy, JitErrorCode::InvalidRange);
}

#[test]
fn rejects_zero_stride_and_excess_budget() {
    let mut policy = valid_policy();
    policy.sample_stride = 0;
    assert_code(policy, JitErrorCode::InvalidStride);

    let mut policy = valid_policy();
    policy.metadata_budget = 257;
    assert_code(policy, JitErrorCode::BudgetExceeded);

    let mut policy = valid_policy();
    policy.metadata_budget = 7;
    assert_code(policy, JitErrorCode::BudgetExceeded);
}

#[test]
fn rejects_version_empty_classes_and_unsupported_class() {
    let mut policy = valid_policy();
    policy.version = 2;
    assert_code(policy, JitErrorCode::PolicyVersion);

    let mut policy = valid_policy();
    policy.allowed_classes.clear();
    assert_code(policy, JitErrorCode::Unsupported);

    let mut policy = valid_policy();
    policy.allowed_classes.push(EventClass::PtxModuleLoad);
    assert_code(policy, JitErrorCode::Unsupported);
}

#[test]
fn rejects_invalid_forward_control_flow() {
    let mut policy = valid_policy();
    policy.rules.insert(
        0,
        Rule::MatchClass {
            class: EventClass::CxlMemWrite,
            skip: 0,
        },
    );
    assert_code(policy, JitErrorCode::InvalidControlFlow);

    let mut policy = valid_policy();
    policy.rules.insert(
        0,
        Rule::MatchClass {
            class: EventClass::CxlMemWrite,
            skip: 31,
        },
    );
    assert_code(policy, JitErrorCode::InvalidControlFlow);

    let mut policy = valid_policy();
    policy.rules.pop();
    assert_code(policy, JitErrorCode::InvalidControlFlow);

    let mut policy = valid_policy();
    policy.rules.insert(2, Rule::Emit);
    assert_code(policy, JitErrorCode::InvalidControlFlow);
}

#[test]
fn rejects_capture_epoch_and_sampling_disagreement() {
    let mut policy = valid_policy();
    policy.rules[0] = Rule::Capture {
        mode: RecordMode::Full,
    };
    assert_code(policy, JitErrorCode::InvalidControlFlow);

    let mut policy = valid_policy();
    policy.rules[2] = Rule::EpochIncrement;
    assert_code(policy, JitErrorCode::InvalidControlFlow);

    let mut policy = valid_policy();
    policy.sample_stride = 2;
    assert_code(policy, JitErrorCode::InvalidControlFlow);

    let mut policy = valid_policy();
    policy.rules.insert(0, Rule::Sample { stride: 0, skip: 1 });
    assert_code(policy, JitErrorCode::InvalidStride);
}

#[test]
fn canonical_digest_ignores_input_whitespace() {
    let policy = valid_policy();
    let pretty = serde_json::to_vec_pretty(&policy).unwrap();
    let compact = serde_json::to_vec(&policy).unwrap();
    let left = Policy::parse(&pretty).unwrap().verify().unwrap();
    let right = Policy::parse(&compact).unwrap().verify().unwrap();

    assert_eq!(left.canonical_json, compact);
    assert_eq!(left.canonical_json, right.canonical_json);
    assert_eq!(left.digest, right.digest);
    assert_ne!(left.digest, [0; 32]);
}
