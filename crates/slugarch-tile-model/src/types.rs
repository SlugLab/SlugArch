use serde::{Deserialize, Serialize};
use std::fmt;

pub const LINE_BYTES: u64 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum EventKind {
    ReadShared = 1,
    ReadExclusive = 2,
    Writeback = 3,
    Invalidate = 4,
    InvalidateAck = 5,
    Fence = 6,
    Completion = 7,
    EpochSeal = 8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum FaultCode {
    CohInvalidatePending = 0x1001,
    CohStaleVersion = 0x1002,
    CohCompletionOrder = 0x1003,
    CohFenceMissing = 0x1004,
    PolicyDigest = 0x2001,
    RecordDrop = 0x2002,
    EvidenceSequence = 0x3001,
    EvidenceCounters = 0x3002,
    EvidenceIncomplete = 0x3003,
    EvidenceModelSeal = 0x3004,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileEvent {
    pub tile_id: u16,
    pub event_id: u64,
    pub request_id: u64,
    pub epoch: u64,
    pub line_address: u64,
    pub version: u64,
    pub kind: EventKind,
}

impl TileEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tile_id: u16,
        event_id: u64,
        request_id: u64,
        epoch: u64,
        line_address: u64,
        version: u64,
        kind: EventKind,
    ) -> Result<Self, ModelError> {
        if tile_id > 63 {
            return Err(ModelError::new(
                0x0001,
                tile_id,
                event_id,
                epoch,
                "tile_id must be in 0..=63",
            ));
        }
        if line_address % LINE_BYTES != 0 {
            return Err(ModelError::new(
                0x0002,
                tile_id,
                event_id,
                epoch,
                "line_address must be 64-byte aligned",
            ));
        }
        Ok(Self {
            tile_id,
            event_id,
            request_id,
            epoch,
            line_address,
            version,
            kind,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LineState {
    pub version: u64,
    pub owner_tile: Option<u16>,
    pub sharers: u64,
    pub last_writer_tile: Option<u16>,
    pub visible_epoch: u64,
    pub outstanding_invalidations: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedEvent {
    pub event: TileEvent,
    pub line_before: Option<LineState>,
    pub line_after: Option<LineState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TileCounters {
    pub event_count: u64,
    pub record_count: u64,
    pub metadata_bytes: u64,
    pub reject_count: u64,
    pub drop_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureRecord {
    pub code: FaultCode,
    pub tile_id: u16,
    pub event_id: u64,
    pub epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelError {
    pub code: u32,
    pub tile_id: u16,
    pub event_id: u64,
    pub epoch: u64,
    pub detail: String,
}

impl ModelError {
    pub fn new(
        code: u32,
        tile_id: u16,
        event_id: u64,
        epoch: u64,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            code,
            tile_id,
            event_id,
            epoch,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for ModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "SlugArch model error 0x{:04x} at tile {} event {} epoch {}: {}",
            self.code, self.tile_id, self.event_id, self.epoch, self.detail
        )
    }
}

impl std::error::Error for ModelError {}
