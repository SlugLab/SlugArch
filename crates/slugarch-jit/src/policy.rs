use crate::{Direction, EventClass, JitError, JitErrorCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddressRange {
    pub base: u64,
    pub length: u64,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordMode {
    Validation = 0,
    Delta = 1,
    Full = 2,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpochPolicy {
    Phase = 0,
    Increment = 1,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Rule {
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub version: u32,
    pub name: String,
    pub allowed_classes: Vec<EventClass>,
    pub ranges: Vec<AddressRange>,
    pub sample_stride: u32,
    pub record_mode: RecordMode,
    pub metadata_budget: u32,
    pub epoch_policy: EpochPolicy,
    pub rules: Vec<Rule>,
}

impl Policy {
    pub fn parse(input: &[u8]) -> Result<Self, JitError> {
        let policy: Self = serde_json::from_slice(input).map_err(|error| {
            JitError::new(
                JitErrorCode::Parse,
                format!("policy JSON is invalid: {error}"),
            )
        })?;

        if policy.ranges.iter().any(|range| range.length == 0) {
            return Err(JitError::new(
                JitErrorCode::InvalidRange,
                "policy range length is zero",
            ));
        }
        Ok(policy)
    }
}
