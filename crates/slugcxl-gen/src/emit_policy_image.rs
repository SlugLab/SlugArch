use serde::Serialize;
use slugarch_jit::{
    AddressRange, Direction, EventClass, Instruction, RecordMode, VerifiedPolicy,
    SLUG_JIT_ABI_VERSION, SLUG_JIT_EVENT_VERSION, SLUG_JIT_PACKET_VERSION,
};

pub const POLICY_HEADER_BYTES: usize = 64;
pub const POLICY_INSTRUCTION_BYTES: usize = 16;
pub const POLICY_INSTRUCTION_SLOTS: usize = 32;
pub const POLICY_RANGE_BYTES: usize = 16;
pub const POLICY_RANGE_SLOTS: usize = 4;
pub const POLICY_IMAGE_BYTES: usize = POLICY_HEADER_BYTES
    + POLICY_INSTRUCTION_SLOTS * POLICY_INSTRUCTION_BYTES
    + POLICY_RANGE_SLOTS * POLICY_RANGE_BYTES;

const OP_HALT: u8 = 0x00;
const OP_MATCH_CLASS: u8 = 0x01;
const OP_MATCH_DIRECTION: u8 = 0x02;
const OP_MATCH_STATUS: u8 = 0x03;
const OP_MATCH_RANGE: u8 = 0x04;
const OP_SAMPLE: u8 = 0x05;
const OP_CAPTURE: u8 = 0x06;
const OP_EMIT: u8 = 0x07;
const OP_EPOCH_INCREMENT: u8 = 0x08;
const OP_EPOCH_FROM_PHASE: u8 = 0x09;
const OP_REJECT: u8 = 0x0a;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PolicyImageError {
    #[error("policy image has length {actual}, expected {expected}")]
    InvalidLength { actual: usize, expected: usize },
    #[error("policy image magic is not SJIT")]
    InvalidMagic,
    #[error("policy image version tuple is unsupported")]
    InvalidVersion,
    #[error("policy image count or byte length is invalid")]
    InvalidCount,
    #[error("policy image reserved or unused bytes are nonzero")]
    ReservedNonzero,
    #[error("policy image digest does not match the verified policy")]
    DigestMismatch,
    #[error("policy image contains unknown opcode {0:#04x}")]
    UnknownOpcode(u8),
    #[error("instruction is unsupported by policy image v1: {0:?}")]
    UnsupportedInstruction(Instruction),
    #[error("policy image field does not encode a known enum value")]
    InvalidEnum,
    #[error("decoded policy image differs from the verified {0}")]
    SemanticMismatch(&'static str),
    #[error("policy manifest JSON is invalid: {0}")]
    Json(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedPolicyImage {
    pub digest: [u8; 32],
    pub instructions: Vec<Instruction>,
    pub ranges: Vec<AddressRange>,
    pub metadata_budget: u32,
}

pub fn encode_policy_image(policy: &VerifiedPolicy) -> Result<Vec<u8>, PolicyImageError> {
    if policy.instructions.len() > POLICY_INSTRUCTION_SLOTS
        || policy.ranges.len() > POLICY_RANGE_SLOTS
    {
        return Err(PolicyImageError::InvalidCount);
    }

    let mut image = vec![0; POLICY_IMAGE_BYTES];
    image[0..4].copy_from_slice(b"SJIT");
    put_u32(&mut image, 0x04, SLUG_JIT_ABI_VERSION);
    put_u32(&mut image, 0x08, SLUG_JIT_EVENT_VERSION);
    put_u32(&mut image, 0x0c, SLUG_JIT_PACKET_VERSION);
    put_u32(&mut image, 0x10, policy.instructions.len() as u32);
    put_u32(&mut image, 0x14, policy.ranges.len() as u32);
    put_u32(&mut image, 0x18, policy.metadata_budget);
    put_u32(&mut image, 0x1c, POLICY_IMAGE_BYTES as u32);
    image[0x20..0x40].copy_from_slice(&policy.digest);

    for (index, instruction) in policy.instructions.iter().copied().enumerate() {
        let offset = POLICY_HEADER_BYTES + index * POLICY_INSTRUCTION_BYTES;
        encode_instruction(
            instruction,
            &mut image[offset..offset + POLICY_INSTRUCTION_BYTES],
        )?;
    }

    let ranges_offset = POLICY_HEADER_BYTES + POLICY_INSTRUCTION_SLOTS * POLICY_INSTRUCTION_BYTES;
    for (index, range) in policy.ranges.iter().enumerate() {
        let offset = ranges_offset + index * POLICY_RANGE_BYTES;
        image[offset..offset + 8].copy_from_slice(&range.base.to_le_bytes());
        image[offset + 8..offset + 16].copy_from_slice(&range.length.to_le_bytes());
    }

    decode_policy_image(&image, policy)?;
    Ok(image)
}

pub fn decode_policy_image(
    image: &[u8],
    expected: &VerifiedPolicy,
) -> Result<DecodedPolicyImage, PolicyImageError> {
    if image.len() != POLICY_IMAGE_BYTES {
        return Err(PolicyImageError::InvalidLength {
            actual: image.len(),
            expected: POLICY_IMAGE_BYTES,
        });
    }
    if &image[0..4] != b"SJIT" {
        return Err(PolicyImageError::InvalidMagic);
    }
    if read_u32(image, 0x04) != SLUG_JIT_ABI_VERSION
        || read_u32(image, 0x08) != SLUG_JIT_EVENT_VERSION
        || read_u32(image, 0x0c) != SLUG_JIT_PACKET_VERSION
    {
        return Err(PolicyImageError::InvalidVersion);
    }

    let instruction_count = read_u32(image, 0x10) as usize;
    let range_count = read_u32(image, 0x14) as usize;
    let metadata_budget = read_u32(image, 0x18);
    if instruction_count > POLICY_INSTRUCTION_SLOTS
        || range_count > POLICY_RANGE_SLOTS
        || read_u32(image, 0x1c) as usize != POLICY_IMAGE_BYTES
    {
        return Err(PolicyImageError::InvalidCount);
    }

    let digest: [u8; 32] = image[0x20..0x40].try_into().expect("fixed digest slice");
    if digest != expected.digest {
        return Err(PolicyImageError::DigestMismatch);
    }

    let mut instructions = Vec::with_capacity(instruction_count);
    for index in 0..instruction_count {
        let offset = POLICY_HEADER_BYTES + index * POLICY_INSTRUCTION_BYTES;
        instructions.push(decode_instruction(
            &image[offset..offset + POLICY_INSTRUCTION_BYTES],
        )?);
    }
    let unused_instruction = POLICY_HEADER_BYTES + instruction_count * POLICY_INSTRUCTION_BYTES;
    let ranges_offset = POLICY_HEADER_BYTES + POLICY_INSTRUCTION_SLOTS * POLICY_INSTRUCTION_BYTES;
    if image[unused_instruction..ranges_offset]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(PolicyImageError::ReservedNonzero);
    }

    let mut ranges = Vec::with_capacity(range_count);
    for index in 0..range_count {
        let offset = ranges_offset + index * POLICY_RANGE_BYTES;
        ranges.push(AddressRange {
            base: read_u64(image, offset),
            length: read_u64(image, offset + 8),
        });
    }
    let unused_range = ranges_offset + range_count * POLICY_RANGE_BYTES;
    if image[unused_range..].iter().any(|byte| *byte != 0) {
        return Err(PolicyImageError::ReservedNonzero);
    }

    if instruction_count != expected.instructions.len() {
        return Err(PolicyImageError::SemanticMismatch("instruction count"));
    }
    if range_count != expected.ranges.len() {
        return Err(PolicyImageError::SemanticMismatch("range count"));
    }
    if instructions != expected.instructions {
        return Err(PolicyImageError::SemanticMismatch("instructions"));
    }
    if ranges != expected.ranges {
        return Err(PolicyImageError::SemanticMismatch("ranges"));
    }
    if metadata_budget != expected.metadata_budget {
        return Err(PolicyImageError::SemanticMismatch("metadata budget"));
    }

    Ok(DecodedPolicyImage {
        digest,
        instructions,
        ranges,
        metadata_budget,
    })
}

pub fn policy_image_hex(image: &[u8]) -> Result<String, PolicyImageError> {
    if image.len() != POLICY_IMAGE_BYTES {
        return Err(PolicyImageError::InvalidLength {
            actual: image.len(),
            expected: POLICY_IMAGE_BYTES,
        });
    }

    let mut output = String::with_capacity(POLICY_IMAGE_BYTES * 2 + POLICY_IMAGE_BYTES / 16);
    for word in image.chunks_exact(16) {
        output.push_str(&word_hex(word));
        output.push('\n');
    }
    Ok(output)
}

pub fn policy_image_manifest(
    policy: &VerifiedPolicy,
    image: &[u8],
) -> Result<String, PolicyImageError> {
    decode_policy_image(image, policy)?;
    let source_policy = serde_json::from_slice::<serde_json::Value>(&policy.canonical_json)
        .map_err(|error| PolicyImageError::Json(error.to_string()))?;
    let words = policy
        .instructions
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let offset = POLICY_HEADER_BYTES + index * POLICY_INSTRUCTION_BYTES;
            word_hex(&image[offset..offset + POLICY_INSTRUCTION_BYTES])
        })
        .collect();
    let manifest = PolicyManifest {
        schema: "slugarch.jit.policy-image.v1",
        abi_version: SLUG_JIT_ABI_VERSION,
        event_version: SLUG_JIT_EVENT_VERSION,
        packet_version: SLUG_JIT_PACKET_VERSION,
        image_bytes: POLICY_IMAGE_BYTES as u32,
        digest: bytes_hex(&policy.digest),
        instruction_words: words,
        ranges: &policy.ranges,
        source_policy,
    };
    serde_json::to_string_pretty(&manifest)
        .map_err(|error| PolicyImageError::Json(error.to_string()))
}

#[derive(Serialize)]
struct PolicyManifest<'a> {
    schema: &'static str,
    abi_version: u32,
    event_version: u32,
    packet_version: u32,
    image_bytes: u32,
    digest: String,
    instruction_words: Vec<String>,
    ranges: &'a [AddressRange],
    source_policy: serde_json::Value,
}

fn encode_instruction(instruction: Instruction, word: &mut [u8]) -> Result<(), PolicyImageError> {
    let (opcode, arg0, skip, arg1) = match instruction {
        Instruction::Halt => (OP_HALT, 0, 0, 0),
        Instruction::MatchClass { class, skip } => {
            (OP_MATCH_CLASS, event_class_code(class), skip, 0)
        }
        Instruction::MatchDirection { direction, skip } => {
            (OP_MATCH_DIRECTION, direction_code(direction), skip, 0)
        }
        Instruction::MatchOpcode { .. } => {
            return Err(PolicyImageError::UnsupportedInstruction(instruction));
        }
        Instruction::MatchStatus { status, skip } => (OP_MATCH_STATUS, 0, skip, status),
        Instruction::MatchRange { range, skip } => (OP_MATCH_RANGE, range, skip, 0),
        Instruction::Sample { stride, skip } => (OP_SAMPLE, 0, skip, stride),
        Instruction::Capture { mode } => (OP_CAPTURE, record_mode_code(mode), 0, 0),
        Instruction::Emit => (OP_EMIT, 0, 0, 0),
        Instruction::EpochIncrement => (OP_EPOCH_INCREMENT, 0, 0, 0),
        Instruction::EpochFromPhase => (OP_EPOCH_FROM_PHASE, 0, 0, 0),
        Instruction::Reject { code } => (OP_REJECT, 0, 0, u32::from(code)),
    };
    word[0] = opcode;
    word[1] = arg0;
    word[2] = skip;
    word[4..8].copy_from_slice(&arg1.to_le_bytes());
    Ok(())
}

fn decode_instruction(word: &[u8]) -> Result<Instruction, PolicyImageError> {
    if word[3] != 0 || word[8..16].iter().any(|byte| *byte != 0) {
        return Err(PolicyImageError::ReservedNonzero);
    }
    let opcode = word[0];
    let arg0 = word[1];
    let skip = word[2];
    let arg1 = read_u32(word, 4);

    let no_arguments = || {
        if arg0 == 0 && skip == 0 && arg1 == 0 {
            Ok(())
        } else {
            Err(PolicyImageError::ReservedNonzero)
        }
    };

    match opcode {
        OP_HALT => {
            no_arguments()?;
            Ok(Instruction::Halt)
        }
        OP_MATCH_CLASS if arg1 == 0 => Ok(Instruction::MatchClass {
            class: decode_event_class(arg0)?,
            skip,
        }),
        OP_MATCH_DIRECTION if arg1 == 0 => Ok(Instruction::MatchDirection {
            direction: decode_direction(arg0)?,
            skip,
        }),
        OP_MATCH_STATUS if arg0 == 0 => Ok(Instruction::MatchStatus { status: arg1, skip }),
        OP_MATCH_RANGE if arg1 == 0 => Ok(Instruction::MatchRange { range: arg0, skip }),
        OP_SAMPLE if arg0 == 0 => Ok(Instruction::Sample { stride: arg1, skip }),
        OP_CAPTURE if skip == 0 && arg1 == 0 => Ok(Instruction::Capture {
            mode: decode_record_mode(arg0)?,
        }),
        OP_EMIT => {
            no_arguments()?;
            Ok(Instruction::Emit)
        }
        OP_EPOCH_INCREMENT => {
            no_arguments()?;
            Ok(Instruction::EpochIncrement)
        }
        OP_EPOCH_FROM_PHASE => {
            no_arguments()?;
            Ok(Instruction::EpochFromPhase)
        }
        OP_REJECT if arg0 == 0 && skip == 0 && arg1 <= u32::from(u16::MAX) => {
            Ok(Instruction::Reject { code: arg1 as u16 })
        }
        _ if opcode <= OP_REJECT => Err(PolicyImageError::ReservedNonzero),
        _ => Err(PolicyImageError::UnknownOpcode(opcode)),
    }
}

fn event_class_code(class: EventClass) -> u8 {
    match class {
        EventClass::CxlMemRead => 1,
        EventClass::CxlMemWrite => 2,
        EventClass::CxlMemData => 3,
        EventClass::Completion => 4,
        EventClass::PtxModuleLoad => 5,
        EventClass::KernelLaunch => 6,
        EventClass::Phase => 7,
        EventClass::Fence => 8,
    }
}

fn decode_event_class(value: u8) -> Result<EventClass, PolicyImageError> {
    match value {
        1 => Ok(EventClass::CxlMemRead),
        2 => Ok(EventClass::CxlMemWrite),
        3 => Ok(EventClass::CxlMemData),
        4 => Ok(EventClass::Completion),
        5 => Ok(EventClass::PtxModuleLoad),
        6 => Ok(EventClass::KernelLaunch),
        7 => Ok(EventClass::Phase),
        8 => Ok(EventClass::Fence),
        _ => Err(PolicyImageError::InvalidEnum),
    }
}

fn direction_code(direction: Direction) -> u8 {
    match direction {
        Direction::HostToDevice => 0,
        Direction::DeviceToHost => 1,
    }
}

fn decode_direction(value: u8) -> Result<Direction, PolicyImageError> {
    match value {
        0 => Ok(Direction::HostToDevice),
        1 => Ok(Direction::DeviceToHost),
        _ => Err(PolicyImageError::InvalidEnum),
    }
}

fn record_mode_code(mode: RecordMode) -> u8 {
    match mode {
        RecordMode::Validation => 0,
        RecordMode::Delta => 1,
        RecordMode::Full => 2,
    }
}

fn decode_record_mode(value: u8) -> Result<RecordMode, PolicyImageError> {
    match value {
        0 => Ok(RecordMode::Validation),
        1 => Ok(RecordMode::Delta),
        2 => Ok(RecordMode::Full),
        _ => Err(PolicyImageError::InvalidEnum),
    }
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32 slice"))
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("u64 slice"))
}

fn bytes_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut output, "{byte:02x}").expect("writing to a string");
    }
    output
}

fn word_hex(word: &[u8]) -> String {
    let mut output = String::with_capacity(word.len() * 2);
    for byte in word.iter().rev() {
        use std::fmt::Write;
        write!(&mut output, "{byte:02x}").expect("writing to a string");
    }
    output
}
