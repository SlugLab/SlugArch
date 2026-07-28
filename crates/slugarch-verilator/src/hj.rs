//! Safe driver for the bounded SlugArch Hardware-JIT RTL.
//!
//! This model uses SlugArch's fixed 64-byte local event packet. That packet is
//! a Verilator/debug transport and is not a standards-compliant CXL FLIT. The
//! Verilated top is compiled in observe-only mode so endpoint responses cannot
//! be mistaken for new policy events; the generated synthesis top defaults to
//! the connected endpoint path.

use std::cell::Cell;
use std::marker::PhantomData;
use std::mem::MaybeUninit;

use slugarch_jit::{
    DeltaPair, Direction, Event, EventClass, PayloadCapture, RecordMode, ReplayRecord,
    VerifiedPolicy, MAX_EVENT_PAYLOAD,
};
use slugarch_verilator_sys as sys;

const EVENT_BYTES: usize = 64;
const RECORD_BYTES: usize = 128;
const RECORD_HEADER_BYTES: usize = 96;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HjStats {
    pub cycles: u64,
    pub event_count: u64,
    pub record_count: u64,
    pub metadata_bytes: u64,
    pub reject_count: u64,
    pub drop_count: u64,
    pub instruction_count: u64,
    pub epoch: u64,
    pub app_flit_bytes: u64,
    pub stall_cycles: u64,
    pub policy_error: u32,
    pub last_reject_code: u16,
    pub policy_ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HjObservation {
    pub accepted: bool,
    pub reject_code: Option<u16>,
    pub record: Option<ReplayRecord>,
    pub record_image: Option<HjRecordImage>,
    pub stats: HjStats,
    pub cycles: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HjRecordImage {
    pub bytes: [u8; RECORD_BYTES],
    pub length: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum HjError {
    #[error("Verilated Hardware-JIT model construction failed")]
    Construction,
    #[error("Hardware-JIT input is not representable by local transport v1: {0}")]
    Unsupported(&'static str),
    #[error("Hardware-JIT policy image failed: {0}")]
    Policy(String),
    #[error("Hardware-JIT operation timed out")]
    Timeout,
    #[error("Hardware-JIT RTL failed with JIT error code {0}")]
    Rtl(u32),
    #[error("Hardware-JIT record exceeded its fixed buffer")]
    Buffer,
    #[error("Hardware-JIT protocol failed: {0}")]
    Protocol(&'static str),
    #[error("Hardware-JIT record is malformed: {0}")]
    Decode(&'static str),
}

/// One exclusively owned Hardware-JIT model and Verilated context.
///
/// Construction performs a reset. A successfully encoded policy must be
/// loaded before [`Self::observe`].
pub struct VerilatedHj {
    raw: *mut sys::SlugarchHj,
    loaded: bool,
    record_mode: Option<RecordMode>,
    _not_sync: PhantomData<Cell<()>>,
}

impl VerilatedHj {
    pub fn new() -> Result<Self, HjError> {
        // SAFETY: the constructor returns one exclusively owned opaque model.
        let raw = unsafe { sys::slugarch_hj_new() };
        if raw.is_null() {
            return Err(HjError::Construction);
        }
        let mut model = Self {
            raw,
            loaded: false,
            record_mode: None,
            _not_sync: PhantomData,
        };
        model.reset()?;
        Ok(model)
    }

    pub fn reset(&mut self) -> Result<(), HjError> {
        // SAFETY: self.raw is live and exclusively owned until Drop.
        unsafe { sys::slugarch_hj_reset(self.raw) };
        self.loaded = false;
        self.record_mode = None;
        let stats = self.stats()?;
        if stats.policy_ready || stats.policy_error != 0 {
            return Err(HjError::Protocol("reset left active policy state"));
        }
        Ok(())
    }

    pub fn load_policy(&mut self, policy: &VerifiedPolicy) -> Result<(), HjError> {
        let image = slugcxl_gen::encode_policy_image(policy)
            .map_err(|error| HjError::Policy(error.to_string()))?;
        // SAFETY: image is a live 640-byte policy buffer for this call.
        let code =
            unsafe { sys::slugarch_hj_load_policy(self.raw, image.as_ptr(), image.len() as u32) };
        self.check_code(code)?;
        let stats = self.stats()?;
        if !stats.policy_ready || stats.policy_error != 0 {
            self.loaded = false;
            self.record_mode = None;
            return Err(HjError::Protocol("policy commit did not become ready"));
        }
        self.loaded = true;
        self.record_mode = Some(policy.record_mode);
        Ok(())
    }

    pub fn observe(&mut self, event: &Event) -> Result<HjObservation, HjError> {
        if !self.loaded {
            return Err(HjError::Protocol("no policy is loaded"));
        }
        event
            .validate()
            .map_err(|_| HjError::Unsupported("event payload is malformed"))?;
        if event.payload_len > 32 {
            return Err(HjError::Unsupported("payload exceeds 32 bytes"));
        }
        if event.opcode > 0x0f {
            return Err(HjError::Unsupported("opcode exceeds four bits"));
        }
        if event.tag > u64::from(u16::MAX) {
            return Err(HjError::Unsupported("tag exceeds 16 bits"));
        }
        if self.record_mode == Some(RecordMode::Delta)
            && event.payload[..usize::from(event.payload_len)]
                .iter()
                .filter(|byte| **byte != 0)
                .count()
                > 16
        {
            return Err(HjError::Unsupported(
                "delta payload exceeds 16 nonzero pairs",
            ));
        }

        let class = match event.class {
            EventClass::CxlMemRead => 1,
            EventClass::CxlMemWrite => 2,
            EventClass::CxlMemData => 3,
            EventClass::Completion => 4,
            _ => return Err(HjError::Unsupported("event class is outside local CXL v1")),
        };
        let direction = match event.direction {
            Direction::HostToDevice => 0,
            Direction::DeviceToHost => 1,
        };
        let mut packet = [0; EVENT_BYTES];
        packet[0] = (class << 4) | event.opcode as u8;
        packet[1..3].copy_from_slice(&(event.tag as u16).to_le_bytes());
        packet[3..11].copy_from_slice(&event.address.to_le_bytes());
        let payload_len = usize::from(event.payload_len);
        packet[11..11 + payload_len].copy_from_slice(&event.payload[..payload_len]);
        packet[43..51].copy_from_slice(&event.event_id.to_le_bytes());
        packet[51..59].copy_from_slice(&event.phase_id.to_le_bytes());
        packet[59..63].copy_from_slice(&event.status.to_le_bytes());
        packet[63] = event.payload_len | (direction << 7);

        let before = self.stats()?;
        let mut record_bytes = [0; RECORD_BYTES];
        let mut record_len = 0_u32;
        let mut cycles = 0_u64;
        // SAFETY: all buffers have the fixed sizes required by the C ABI.
        let code = unsafe {
            sys::slugarch_hj_observe(
                self.raw,
                packet.as_ptr(),
                record_bytes.as_mut_ptr(),
                &mut record_len,
                &mut cycles,
            )
        };
        self.check_code(code)?;
        let stats = self.stats()?;
        if stats.event_count
            != before
                .event_count
                .checked_add(1)
                .ok_or(HjError::Protocol("event counter overflowed"))?
        {
            return Err(HjError::Protocol(
                "observe did not commit exactly one event",
            ));
        }
        if stats.drop_count != before.drop_count {
            return Err(HjError::Rtl(stats.policy_error));
        }

        let (record, record_image) = if record_len == 0 {
            (None, None)
        } else {
            let length = record_len as usize;
            (
                Some(decode_record(&record_bytes, length)?),
                Some(HjRecordImage {
                    bytes: record_bytes,
                    length,
                }),
            )
        };
        let next_reject = before
            .reject_count
            .checked_add(1)
            .ok_or(HjError::Protocol("reject counter overflowed"))?;
        let reject_code = if stats.reject_count == before.reject_count {
            None
        } else if stats.reject_count == next_reject && record.is_none() {
            Some(stats.last_reject_code)
        } else {
            return Err(HjError::Protocol("reject counter transition is invalid"));
        };

        Ok(HjObservation {
            accepted: reject_code.is_none(),
            reject_code,
            record,
            record_image,
            stats,
            cycles,
        })
    }

    pub fn stats(&self) -> Result<HjStats, HjError> {
        let mut raw = MaybeUninit::<sys::SlugarchHjStats>::uninit();
        // SAFETY: raw points to writable storage and self.raw is a live model.
        let code = unsafe { sys::slugarch_hj_stats(self.raw, raw.as_mut_ptr()) };
        if code != sys::SLUGARCH_HJ_OK {
            return Err(map_code(code, 0));
        }
        // SAFETY: the successful C call initialized every field.
        let raw = unsafe { raw.assume_init() };
        Ok(HjStats {
            cycles: raw.cycles,
            event_count: raw.event_count,
            record_count: raw.record_count,
            metadata_bytes: raw.metadata_bytes,
            reject_count: raw.reject_count,
            drop_count: raw.drop_count,
            instruction_count: raw.instruction_count,
            epoch: raw.epoch,
            app_flit_bytes: raw.app_flit_bytes,
            stall_cycles: raw.stall_cycles,
            policy_error: raw.policy_error,
            last_reject_code: raw.last_reject_code,
            policy_ready: raw.policy_ready != 0,
        })
    }

    fn check_code(&mut self, code: i32) -> Result<(), HjError> {
        if code == sys::SLUGARCH_HJ_OK {
            return Ok(());
        }
        let policy_error = self.stats().map_or(0, |stats| {
            self.loaded = stats.policy_ready && stats.policy_error == 0;
            if !self.loaded {
                self.record_mode = None;
            }
            stats.policy_error
        });
        Err(map_code(code, policy_error))
    }
}

impl Drop for VerilatedHj {
    fn drop(&mut self) {
        // SAFETY: self.raw was allocated by slugarch_hj_new and is freed once.
        unsafe { sys::slugarch_hj_free(self.raw) };
        self.raw = std::ptr::null_mut();
    }
}

// SAFETY: each wrapper exclusively owns one VerilatedContext and model. Cell
// keeps the type !Sync; moving exclusive ownership to another thread is safe.
unsafe impl Send for VerilatedHj {}

fn map_code(code: i32, policy_error: u32) -> HjError {
    match code {
        value if value == sys::SLUGARCH_HJ_ERR_TIMEOUT => HjError::Timeout,
        value if value == sys::SLUGARCH_HJ_ERR_RTL => HjError::Rtl(policy_error),
        value if value == sys::SLUGARCH_HJ_ERR_BUFFER => HjError::Buffer,
        value if value == sys::SLUGARCH_HJ_ERR_SIZE => HjError::Protocol("size mismatch"),
        value if value == sys::SLUGARCH_HJ_ERR_NULL => HjError::Protocol("null pointer"),
        value if value == sys::SLUGARCH_HJ_ERR_PROTOCOL => {
            HjError::Protocol("C/RTL handshake mismatch")
        }
        _ => HjError::Protocol("unknown C shim error"),
    }
}

fn decode_record(bytes: &[u8; RECORD_BYTES], length: usize) -> Result<ReplayRecord, HjError> {
    if !(RECORD_HEADER_BYTES..=RECORD_BYTES).contains(&length)
        || bytes[0..3] != [1, 1, 1]
        || bytes[3] != 0
        || bytes[88..96].iter().any(|byte| *byte != 0)
        || bytes[length..].iter().any(|byte| *byte != 0)
    {
        return Err(HjError::Decode("header, length, or reserved bytes"));
    }
    let payload_len = bytes[85];
    let capture_len = usize::from(bytes[86]);
    let pair_count = usize::from(bytes[87]);
    if payload_len > 32 || length != RECORD_HEADER_BYTES + capture_len {
        return Err(HjError::Decode("capture length"));
    }

    let payload = match bytes[84] {
        0 if capture_len == 8 && pair_count == 0 => PayloadCapture::Validation {
            length: payload_len,
            hash: read_u64(bytes, 96),
        },
        1 if capture_len == pair_count * 2 && pair_count <= 16 => {
            let mut pairs = [DeltaPair::default(); MAX_EVENT_PAYLOAD];
            let mut last_index = None;
            for (slot, pair) in pairs.iter_mut().take(pair_count).enumerate() {
                let index = bytes[96 + slot * 2];
                let value = bytes[97 + slot * 2];
                if usize::from(index) >= usize::from(payload_len)
                    || value == 0
                    || last_index.is_some_and(|last| index <= last)
                {
                    return Err(HjError::Decode("delta pair"));
                }
                *pair = DeltaPair { index, value };
                last_index = Some(index);
            }
            PayloadCapture::Delta {
                length: payload_len,
                pair_count: pair_count as u8,
                pairs,
            }
        }
        2 if capture_len == usize::from(payload_len) && pair_count == 0 => {
            let mut payload = [0; MAX_EVENT_PAYLOAD];
            payload[..capture_len].copy_from_slice(&bytes[96..96 + capture_len]);
            PayloadCapture::Full {
                length: payload_len,
                bytes: payload,
            }
        }
        _ => return Err(HjError::Decode("capture mode")),
    };

    Ok(ReplayRecord {
        sequence: read_u64(bytes, 4),
        event_id: read_u64(bytes, 12),
        policy_digest: bytes[20..52]
            .try_into()
            .map_err(|_| HjError::Decode("policy digest"))?,
        epoch: read_u64(bytes, 52),
        direction: match bytes[60] {
            0 => Direction::HostToDevice,
            1 => Direction::DeviceToHost,
            _ => return Err(HjError::Decode("direction")),
        },
        class: match bytes[61] {
            1 => EventClass::CxlMemRead,
            2 => EventClass::CxlMemWrite,
            3 => EventClass::CxlMemData,
            4 => EventClass::Completion,
            _ => return Err(HjError::Decode("event class")),
        },
        opcode: read_u16(bytes, 62),
        address: read_u64(bytes, 64),
        tag: read_u64(bytes, 72),
        status: read_u32(bytes, 80),
        payload,
    })
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("fixed u16 slice"),
    )
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("fixed u32 slice"),
    )
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("fixed u64 slice"),
    )
}
