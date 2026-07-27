use crate::{
    DeltaPair, Event, Instruction, JitError, JitErrorCode, PayloadCapture, RecordMode,
    ReplayRecord, VerifiedPolicy, MAX_EVENT_PAYLOAD, MAX_INSTRUCTIONS,
};

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x00000100000001b3;

#[derive(Debug, Clone, PartialEq, Eq)]
// Keep the replay record inline so observing an event never allocates.
#[allow(clippy::large_enum_variant)]
pub enum Decision {
    Accept,
    Emit { record: ReplayRecord },
    Reject { code: u16 },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    pub event_count: u64,
    pub record_count: u64,
    pub metadata_bytes: u64,
    pub reject_count: u64,
    pub drop_count: u64,
    pub instruction_count: u64,
    pub epoch: u64,
}

#[derive(Debug)]
pub struct Engine {
    policy: VerifiedPolicy,
    epoch: u64,
    sequence: u64,
    stats: Stats,
    last_record: Option<ReplayRecord>,
    poisoned: bool,
}

impl Engine {
    pub fn new(policy: VerifiedPolicy) -> Self {
        Self {
            policy,
            epoch: 0,
            sequence: 0,
            stats: Stats::default(),
            last_record: None,
            poisoned: false,
        }
    }

    pub fn policy(&self) -> &VerifiedPolicy {
        &self.policy
    }

    pub fn stats(&self) -> Stats {
        self.stats
    }

    pub fn last_record(&self) -> Option<&ReplayRecord> {
        self.last_record.as_ref()
    }

    fn poison(&mut self, error: JitError) -> JitError {
        self.poisoned = true;
        error
    }

    pub fn observe(&mut self, event: &Event) -> Result<Decision, JitError> {
        if self.poisoned {
            return Err(JitError::new(
                JitErrorCode::Poisoned,
                "JIT engine is poisoned",
            ));
        }
        if let Err(error) = event.validate() {
            return Err(self.poison(error));
        }
        if !self.policy.allowed_classes.contains(&event.class) {
            return Err(self.poison(JitError::new(
                JitErrorCode::Unsupported,
                "event class is not allowed by the policy",
            )));
        }

        match self.execute(event) {
            Ok(decision) => Ok(decision),
            Err(error) => Err(self.poison(error)),
        }
    }

    fn execute(&mut self, event: &Event) -> Result<Decision, JitError> {
        let mut pc = 0usize;
        let mut steps = 0u64;
        let mut epoch = self.epoch;
        let mut capture = None;
        let mut should_emit = false;

        loop {
            if steps >= MAX_INSTRUCTIONS as u64 {
                return Err(JitError::new(
                    JitErrorCode::Timeout,
                    "policy execution exceeded 32 instructions",
                ));
            }
            let instruction = self.policy.instructions.get(pc).copied().ok_or_else(|| {
                JitError::new(
                    JitErrorCode::InvalidControlFlow,
                    "policy counter left the verified program",
                )
            })?;
            steps += 1;

            match instruction {
                Instruction::MatchClass { class, skip } => {
                    pc = branch(pc, event.class == class, skip);
                }
                Instruction::MatchDirection { direction, skip } => {
                    pc = branch(pc, event.direction == direction, skip);
                }
                Instruction::MatchOpcode { opcode, skip } => {
                    pc = branch(pc, event.opcode == opcode, skip);
                }
                Instruction::MatchStatus { status, skip } => {
                    pc = branch(pc, event.status == status, skip);
                }
                Instruction::MatchRange { range, skip } => {
                    let range = self.policy.ranges[usize::from(range)];
                    let matched =
                        event.address >= range.base && event.address - range.base < range.length;
                    pc = branch(pc, matched, skip);
                }
                Instruction::Sample { stride, skip } => {
                    pc = branch(pc, event.event_id % u64::from(stride) == 0, skip);
                }
                Instruction::Capture { mode } => {
                    capture = Some(capture_payload(event, mode));
                    pc += 1;
                }
                Instruction::Emit => {
                    should_emit = true;
                    pc += 1;
                }
                Instruction::EpochIncrement => {
                    epoch = epoch.checked_add(1).ok_or_else(|| {
                        JitError::new(JitErrorCode::Backend, "epoch counter overflowed")
                    })?;
                    pc += 1;
                }
                Instruction::EpochFromPhase => {
                    epoch = event.phase_id;
                    pc += 1;
                }
                Instruction::Reject { code } => {
                    let mut stats = self.stats;
                    stats.event_count = checked_add(stats.event_count, 1, "event")?;
                    stats.reject_count = checked_add(stats.reject_count, 1, "reject")?;
                    stats.instruction_count =
                        checked_add(stats.instruction_count, steps, "instruction")?;
                    stats.epoch = epoch;
                    self.epoch = epoch;
                    self.stats = stats;
                    return Ok(Decision::Reject { code });
                }
                Instruction::Halt => break,
            }
        }

        let mut stats = self.stats;
        stats.event_count = checked_add(stats.event_count, 1, "event")?;
        stats.instruction_count = checked_add(stats.instruction_count, steps, "instruction")?;
        stats.epoch = epoch;
        let decision = if should_emit {
            let payload = capture.ok_or_else(|| {
                JitError::new(
                    JitErrorCode::InvalidControlFlow,
                    "emit reached without a payload capture",
                )
            })?;
            let sequence = checked_add(self.sequence, 1, "record sequence")?;
            let record = ReplayRecord {
                sequence,
                event_id: event.event_id,
                policy_digest: self.policy.digest,
                epoch,
                direction: event.direction,
                class: event.class,
                opcode: event.opcode,
                address: event.address,
                tag: event.tag,
                status: event.status,
                payload,
            };
            stats.record_count = checked_add(stats.record_count, 1, "record")?;
            stats.metadata_bytes = checked_add(
                stats.metadata_bytes,
                record.payload.captured_bytes(),
                "metadata byte",
            )?;
            self.sequence = sequence;
            self.last_record = Some(record.clone());
            Decision::Emit { record }
        } else {
            Decision::Accept
        };

        self.epoch = epoch;
        self.stats = stats;
        Ok(decision)
    }
}

fn branch(pc: usize, matched: bool, skip: u8) -> usize {
    if matched {
        pc + 1
    } else {
        pc + 1 + usize::from(skip)
    }
}

fn checked_add(value: u64, increment: u64, name: &str) -> Result<u64, JitError> {
    value
        .checked_add(increment)
        .ok_or_else(|| JitError::new(JitErrorCode::Backend, format!("{name} counter overflowed")))
}

fn capture_payload(event: &Event, mode: RecordMode) -> PayloadCapture {
    let length = event.payload_len;
    let payload = &event.payload[..usize::from(length)];

    match mode {
        RecordMode::Validation => {
            let hash = payload.iter().fold(FNV_OFFSET, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
            });
            PayloadCapture::Validation { length, hash }
        }
        RecordMode::Delta => {
            let mut pairs = [DeltaPair::default(); MAX_EVENT_PAYLOAD];
            let mut pair_count = 0usize;

            for (index, value) in payload.iter().copied().enumerate() {
                if value != 0 {
                    pairs[pair_count] = DeltaPair {
                        index: index as u8,
                        value,
                    };
                    pair_count += 1;
                }
            }
            PayloadCapture::Delta {
                length,
                pair_count: pair_count as u8,
                pairs,
            }
        }
        RecordMode::Full => {
            let mut bytes = [0; MAX_EVENT_PAYLOAD];
            bytes[..payload.len()].copy_from_slice(payload);
            PayloadCapture::Full { length, bytes }
        }
    }
}
