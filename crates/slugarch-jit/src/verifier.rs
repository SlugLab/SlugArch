use sha2::{Digest, Sha256};

use crate::program::MAX_METADATA_BYTES;
use crate::{
    EpochPolicy, EventClass, Instruction, JitError, JitErrorCode, Policy, RecordMode, Rule,
    VerifiedPolicy, MAX_EVENT_PAYLOAD, MAX_INSTRUCTIONS, MAX_RANGES, SLUG_JIT_ABI_VERSION,
    SLUG_JIT_BACKEND_CONTRACT_VERSION, SLUG_JIT_EVENT_VERSION, SLUG_JIT_PACKET_VERSION,
};

fn supported_class(class: EventClass) -> bool {
    matches!(
        class,
        EventClass::CxlMemRead
            | EventClass::CxlMemWrite
            | EventClass::CxlMemData
            | EventClass::Completion
            | EventClass::Phase
            | EventClass::Fence
    )
}

fn capture_bytes(mode: RecordMode) -> u32 {
    match mode {
        RecordMode::Validation => 8,
        RecordMode::Delta | RecordMode::Full => MAX_EVENT_PAYLOAD as u32,
    }
}

fn invalid_control_flow(message: impl Into<String>) -> JitError {
    JitError::new(JitErrorCode::InvalidControlFlow, message)
}

fn compile_rule(rule: &Rule) -> Instruction {
    match *rule {
        Rule::MatchClass { class, skip } => Instruction::MatchClass { class, skip },
        Rule::MatchDirection { direction, skip } => Instruction::MatchDirection { direction, skip },
        Rule::MatchOpcode { opcode, skip } => Instruction::MatchOpcode { opcode, skip },
        Rule::MatchStatus { status, skip } => Instruction::MatchStatus { status, skip },
        Rule::MatchRange { range, skip } => Instruction::MatchRange { range, skip },
        Rule::Sample { stride, skip } => Instruction::Sample { stride, skip },
        Rule::Capture { mode } => Instruction::Capture { mode },
        Rule::Emit => Instruction::Emit,
        Rule::EpochIncrement => Instruction::EpochIncrement,
        Rule::EpochFromPhase => Instruction::EpochFromPhase,
        Rule::Reject { code } => Instruction::Reject { code },
        Rule::Halt => Instruction::Halt,
    }
}

fn path_terminates(instructions: &[Instruction], pc: usize) -> bool {
    let Some(instruction) = instructions.get(pc).copied() else {
        return false;
    };

    match instruction {
        Instruction::Halt | Instruction::Reject { .. } => true,
        _ => {
            let fallthrough = path_terminates(instructions, pc + 1);
            if let Some(skip) = instruction.branch_skip() {
                let target = pc + 1 + usize::from(skip);
                fallthrough && path_terminates(instructions, target)
            } else {
                fallthrough
            }
        }
    }
}

impl Policy {
    pub fn verify(&self) -> Result<VerifiedPolicy, JitError> {
        if self.version != SLUG_JIT_ABI_VERSION {
            return Err(JitError::new(
                JitErrorCode::PolicyVersion,
                "policy version is not 1",
            ));
        }
        if self.rules.len() > MAX_INSTRUCTIONS {
            return Err(JitError::new(
                JitErrorCode::TooManyInstructions,
                "policy has more than 32 instructions",
            ));
        }
        if self.ranges.len() > MAX_RANGES {
            return Err(JitError::new(
                JitErrorCode::TooManyRanges,
                "policy has more than four ranges",
            ));
        }
        if self.allowed_classes.is_empty()
            || self
                .allowed_classes
                .iter()
                .copied()
                .any(|class| !supported_class(class))
        {
            return Err(JitError::new(
                JitErrorCode::Unsupported,
                "policy contains no classes or an unsupported class",
            ));
        }
        if self
            .ranges
            .iter()
            .any(|range| range.length == 0 || range.base.checked_add(range.length).is_none())
        {
            return Err(JitError::new(
                JitErrorCode::InvalidRange,
                "policy range is empty or wraps",
            ));
        }
        if self.sample_stride == 0 {
            return Err(JitError::new(
                JitErrorCode::InvalidStride,
                "policy sample stride is zero",
            ));
        }
        if self.metadata_budget > MAX_METADATA_BYTES {
            return Err(JitError::new(
                JitErrorCode::BudgetExceeded,
                "policy metadata budget exceeds 256 bytes",
            ));
        }

        let instructions: Vec<_> = self.rules.iter().map(compile_rule).collect();
        let mut capture_count = 0;
        let mut emit_count = 0;
        let mut halt_count = 0;
        let mut epoch_count = 0;
        let mut sample_count = 0;

        for (index, instruction) in instructions.iter().copied().enumerate() {
            if let Some(skip) = instruction.branch_skip() {
                if skip == 0 || index + 1 + usize::from(skip) >= instructions.len() {
                    return Err(invalid_control_flow(
                        "branch skip is zero or leaves the program",
                    ));
                }
            }
            match instruction {
                Instruction::MatchClass { class, .. } if !supported_class(class) => {
                    return Err(JitError::new(
                        JitErrorCode::Unsupported,
                        "match uses an unsupported event class",
                    ));
                }
                Instruction::MatchRange { range, .. }
                    if usize::from(range) >= self.ranges.len() =>
                {
                    return Err(invalid_control_flow("match range index is invalid"));
                }
                Instruction::Sample { stride, .. } => {
                    if stride == 0 {
                        return Err(JitError::new(
                            JitErrorCode::InvalidStride,
                            "sample instruction stride is zero",
                        ));
                    }
                    if stride != self.sample_stride {
                        return Err(invalid_control_flow(
                            "sample instruction disagrees with policy stride",
                        ));
                    }
                    sample_count += 1;
                }
                Instruction::Capture { mode } => {
                    if mode != self.record_mode {
                        return Err(invalid_control_flow(
                            "capture mode disagrees with policy record mode",
                        ));
                    }
                    if capture_bytes(mode) > self.metadata_budget {
                        return Err(JitError::new(
                            JitErrorCode::BudgetExceeded,
                            "capture exceeds the policy metadata budget",
                        ));
                    }
                    capture_count += 1;
                }
                Instruction::Emit => emit_count += 1,
                Instruction::EpochIncrement => {
                    if self.epoch_policy != EpochPolicy::Increment {
                        return Err(invalid_control_flow(
                            "epoch increment disagrees with policy",
                        ));
                    }
                    epoch_count += 1;
                }
                Instruction::EpochFromPhase => {
                    if self.epoch_policy != EpochPolicy::Phase {
                        return Err(invalid_control_flow("phase epoch disagrees with policy"));
                    }
                    epoch_count += 1;
                }
                Instruction::Reject { code: 0 } => {
                    return Err(invalid_control_flow("reject code is zero"));
                }
                Instruction::Halt => halt_count += 1,
                _ => {}
            }
        }

        if capture_count != 1
            || emit_count > 1
            || halt_count != 1
            || epoch_count != 1
            || sample_count > 1
        {
            return Err(invalid_control_flow(
                "policy capture, emit, epoch, sample, or halt count is invalid",
            ));
        }
        if self.sample_stride != 1 && sample_count != 1 {
            return Err(invalid_control_flow(
                "non-unit sample stride lacks a sample instruction",
            ));
        }
        if instructions.is_empty() || !path_terminates(&instructions, 0) {
            return Err(invalid_control_flow(
                "a reachable policy path does not terminate",
            ));
        }

        let canonical_json = serde_json::to_vec(self).map_err(|error| {
            JitError::new(
                JitErrorCode::Parse,
                format!("policy canonicalization failed: {error}"),
            )
        })?;
        let mut hasher = Sha256::new();
        hasher.update(SLUG_JIT_ABI_VERSION.to_le_bytes());
        hasher.update(SLUG_JIT_EVENT_VERSION.to_le_bytes());
        hasher.update(SLUG_JIT_PACKET_VERSION.to_le_bytes());
        hasher.update(SLUG_JIT_BACKEND_CONTRACT_VERSION.to_le_bytes());
        hasher.update(&canonical_json);
        let digest = hasher.finalize().into();

        Ok(VerifiedPolicy {
            canonical_json,
            digest,
            instructions,
            ranges: self.ranges.clone(),
            allowed_classes: self.allowed_classes.clone(),
            sample_stride: self.sample_stride,
            record_mode: self.record_mode,
            epoch_policy: self.epoch_policy,
            metadata_budget: self.metadata_budget,
        })
    }
}
