use crate::{JitError, JitErrorCode};

pub const MAX_EVENT_PAYLOAD: usize = 64;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    HostToDevice = 0,
    DeviceToHost = 1,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventClass {
    CxlMemRead = 1,
    CxlMemWrite = 2,
    CxlMemData = 3,
    Completion = 4,
    PtxModuleLoad = 5,
    KernelLaunch = 6,
    Phase = 7,
    Fence = 8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub event_id: u64,
    pub client_id: u64,
    pub direction: Direction,
    pub class: EventClass,
    pub opcode: u16,
    pub address: u64,
    pub payload_len: u8,
    pub payload: [u8; MAX_EVENT_PAYLOAD],
    pub tag: u64,
    pub phase_id: u64,
    pub monotonic_ns: u64,
    pub status: u32,
}

impl Event {
    pub fn validate(&self) -> Result<(), JitError> {
        let payload_len = usize::from(self.payload_len);

        if payload_len > MAX_EVENT_PAYLOAD {
            return Err(JitError::new(
                JitErrorCode::Unsupported,
                "event payload length exceeds 64 bytes",
            ));
        }
        if self.payload[payload_len..].iter().any(|byte| *byte != 0) {
            return Err(JitError::new(
                JitErrorCode::Unsupported,
                "event payload tail is nonzero",
            ));
        }
        Ok(())
    }
}
