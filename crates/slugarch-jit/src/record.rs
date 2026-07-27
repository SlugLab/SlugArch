use crate::{Direction, EventClass, MAX_EVENT_PAYLOAD};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeltaPair {
    pub index: u8,
    pub value: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayloadCapture {
    Validation {
        length: u8,
        hash: u64,
    },
    Delta {
        length: u8,
        pair_count: u8,
        pairs: [DeltaPair; MAX_EVENT_PAYLOAD],
    },
    Full {
        length: u8,
        bytes: [u8; MAX_EVENT_PAYLOAD],
    },
}

impl PayloadCapture {
    pub fn captured_bytes(&self) -> u64 {
        match self {
            Self::Validation { .. } => 8,
            Self::Delta { pair_count, .. } => u64::from(*pair_count) * 2,
            Self::Full { length, .. } => u64::from(*length),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayRecord {
    pub sequence: u64,
    pub event_id: u64,
    pub policy_digest: [u8; 32],
    pub epoch: u64,
    pub direction: Direction,
    pub class: EventClass,
    pub opcode: u16,
    pub address: u64,
    pub tag: u64,
    pub status: u32,
    pub payload: PayloadCapture,
}

impl ReplayRecord {
    pub fn encoded_len(&self) -> u32 {
        96 + u32::try_from(self.payload.captured_bytes()).unwrap_or(u32::MAX)
    }
}
