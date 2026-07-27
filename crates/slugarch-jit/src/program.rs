use crate::{AddressRange, Direction, EpochPolicy, EventClass, RecordMode};

pub const MAX_INSTRUCTIONS: usize = 32;
pub const MAX_RANGES: usize = 4;
pub const MAX_METADATA_BYTES: u32 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instruction {
    MatchClass { class: EventClass, skip: u8 },
    MatchDirection { direction: Direction, skip: u8 },
    MatchOpcode { opcode: u16, skip: u8 },
    MatchStatus { status: u32, skip: u8 },
    MatchRange { range: u8, skip: u8 },
    Sample { stride: u32, skip: u8 },
    Capture { mode: RecordMode },
    Emit,
    EpochIncrement,
    EpochFromPhase,
    Reject { code: u16 },
    Halt,
}

impl Instruction {
    pub fn branch_skip(self) -> Option<u8> {
        match self {
            Self::MatchClass { skip, .. }
            | Self::MatchDirection { skip, .. }
            | Self::MatchOpcode { skip, .. }
            | Self::MatchStatus { skip, .. }
            | Self::MatchRange { skip, .. }
            | Self::Sample { skip, .. } => Some(skip),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPolicy {
    pub canonical_json: Vec<u8>,
    pub digest: [u8; 32],
    pub instructions: Vec<Instruction>,
    pub ranges: Vec<AddressRange>,
    pub allowed_classes: Vec<EventClass>,
    pub sample_stride: u32,
    pub record_mode: RecordMode,
    pub epoch_policy: EpochPolicy,
    pub metadata_budget: u32,
}
