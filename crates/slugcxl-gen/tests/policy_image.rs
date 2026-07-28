use slugarch_jit::{
    AddressRange, EpochPolicy, EventClass, Instruction, Policy, RecordMode, Rule, VerifiedPolicy,
    SLUG_JIT_ABI_VERSION,
};
use slugcxl_gen::{
    decode_policy_image, encode_policy_image, PolicyImageError, POLICY_HEADER_BYTES,
    POLICY_IMAGE_BYTES, POLICY_INSTRUCTION_SLOTS, POLICY_RANGE_SLOTS,
};

fn validation_policy_with_classes(
    allowed_classes: Vec<EventClass>,
    rules: Vec<Rule>,
) -> VerifiedPolicy {
    Policy {
        version: SLUG_JIT_ABI_VERSION,
        name: "policy-image-test".to_string(),
        allowed_classes,
        ranges: vec![AddressRange {
            base: 80 * 1024 * 1024,
            length: 32 * 1024 * 1024,
        }],
        sample_stride: 1,
        record_mode: RecordMode::Validation,
        metadata_budget: 256,
        epoch_policy: EpochPolicy::Phase,
        rules,
    }
    .verify()
    .unwrap()
}

fn validation_policy(rules: Vec<Rule>) -> VerifiedPolicy {
    validation_policy_with_classes(
        vec![
            EventClass::CxlMemRead,
            EventClass::CxlMemWrite,
            EventClass::CxlMemData,
            EventClass::Completion,
        ],
        rules,
    )
}

fn matched_policy() -> VerifiedPolicy {
    validation_policy(vec![
        Rule::MatchClass {
            class: EventClass::CxlMemWrite,
            skip: 3,
        },
        Rule::Capture {
            mode: RecordMode::Validation,
        },
        Rule::Emit,
        Rule::EpochFromPhase,
        Rule::Halt,
    ])
}

#[test]
fn verified_policy_has_one_exact_little_endian_image() {
    let verified = matched_policy();
    let image = encode_policy_image(&verified).unwrap();

    assert_eq!(image.len(), POLICY_IMAGE_BYTES);
    assert_eq!(&image[0..4], b"SJIT");
    assert_eq!(u32::from_le_bytes(image[4..8].try_into().unwrap()), 1);
    assert_eq!(
        u32::from_le_bytes(image[16..20].try_into().unwrap()),
        verified.instructions.len() as u32
    );
    assert_eq!(
        u32::from_le_bytes(image[28..32].try_into().unwrap()),
        POLICY_IMAGE_BYTES as u32
    );
    assert_eq!(&image[0x20..0x40], &verified.digest);

    assert_eq!(image[POLICY_HEADER_BYTES], 0x01);
    assert_eq!(
        image[POLICY_HEADER_BYTES + 1],
        EventClass::CxlMemWrite as u8
    );
    assert_eq!(image[POLICY_HEADER_BYTES + 2], 3);
    assert!(image[POLICY_HEADER_BYTES + 3..POLICY_HEADER_BYTES + 16]
        .iter()
        .all(|byte| *byte == 0));

    let unused_instruction = POLICY_HEADER_BYTES + verified.instructions.len() * 16;
    let range_offset = POLICY_HEADER_BYTES + POLICY_INSTRUCTION_SLOTS * 16;
    assert!(image[unused_instruction..range_offset]
        .iter()
        .all(|byte| *byte == 0));
    let unused_range = range_offset + verified.ranges.len() * 16;
    assert!(image[unused_range..range_offset + POLICY_RANGE_SLOTS * 16]
        .iter()
        .all(|byte| *byte == 0));

    let decoded = decode_policy_image(&image, &verified).unwrap();
    assert_eq!(decoded.instructions, verified.instructions);
    assert_eq!(decoded.ranges, verified.ranges);
    assert_eq!(decoded.digest, verified.digest);
    assert_eq!(decoded.metadata_budget, 256);
}

#[test]
fn decoder_rejects_version_reserved_digest_and_unused_slot_drift() {
    let verified = matched_policy();
    let image = encode_policy_image(&verified).unwrap();

    let mut bad_version = image.clone();
    bad_version[4..8].copy_from_slice(&2_u32.to_le_bytes());
    assert!(decode_policy_image(&bad_version, &verified).is_err());

    let mut bad_reserved = image.clone();
    bad_reserved[POLICY_HEADER_BYTES + 3] = 1;
    assert!(decode_policy_image(&bad_reserved, &verified).is_err());

    let mut bad_known_opcode_fields = image.clone();
    bad_known_opcode_fields[POLICY_HEADER_BYTES + 4] = 1;
    assert!(decode_policy_image(&bad_known_opcode_fields, &verified).is_err());

    let mut bad_digest = image.clone();
    bad_digest[0x20] ^= 1;
    assert!(decode_policy_image(&bad_digest, &verified).is_err());

    let mut bad_unused = image;
    bad_unused[POLICY_HEADER_BYTES + verified.instructions.len() * 16] = 1;
    assert!(decode_policy_image(&bad_unused, &verified).is_err());
}

#[test]
fn match_opcode_is_explicitly_unsupported_by_encoding_v1() {
    let verified = validation_policy(vec![
        Rule::MatchOpcode {
            opcode: 0x44,
            skip: 3,
        },
        Rule::Capture {
            mode: RecordMode::Validation,
        },
        Rule::Emit,
        Rule::EpochFromPhase,
        Rule::Halt,
    ]);

    assert!(matches!(
        encode_policy_image(&verified),
        Err(PolicyImageError::UnsupportedInstruction(
            Instruction::MatchOpcode { .. }
        ))
    ));
}

#[test]
fn policy_image_v1_rejects_an_allowlist_it_cannot_serialize() {
    let verified = validation_policy_with_classes(
        vec![EventClass::CxlMemWrite],
        vec![
            Rule::Capture {
                mode: RecordMode::Validation,
            },
            Rule::Emit,
            Rule::EpochFromPhase,
            Rule::Halt,
        ],
    );

    assert!(matches!(
        encode_policy_image(&verified),
        Err(PolicyImageError::UnsupportedAllowedClasses)
    ));
}

#[test]
fn policy_image_v1_rejects_a_match_class_outside_its_fixed_domain() {
    let verified = validation_policy(vec![
        Rule::MatchClass {
            class: EventClass::Phase,
            skip: 3,
        },
        Rule::Capture {
            mode: RecordMode::Validation,
        },
        Rule::Emit,
        Rule::EpochFromPhase,
        Rule::Halt,
    ]);

    assert!(matches!(
        encode_policy_image(&verified),
        Err(PolicyImageError::UnsupportedInstruction(
            Instruction::MatchClass {
                class: EventClass::Phase,
                ..
            }
        ))
    ));
}
