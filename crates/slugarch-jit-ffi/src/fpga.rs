//! Explicit FPGA-Verilator policy backend.
//!
//! This adapter never falls back to the Rust interpreter. It accepts only the
//! local event/image domain enforced by `VerilatedHj` and maps RTL failures to
//! the stable SlugArch JIT error codes.

use slugarch_jit::{Decision, Event, JitError, JitErrorCode, Stats, VerifiedPolicy};
use slugarch_verilator::{HjError, HjRecordImage, HjStats, VerilatedHj};

pub struct FpgaBackend {
    model: VerilatedHj,
    stats: Stats,
    hardware_stats: HjStats,
    last_record_image: Option<HjRecordImage>,
    last_policy_load_cycles: u64,
    last_observation_cycles: u64,
}

impl FpgaBackend {
    pub fn new() -> Result<Self, JitError> {
        let model = VerilatedHj::new().map_err(map_hj_error)?;
        let hardware_stats = model.stats().map_err(map_hj_error)?;
        Ok(Self {
            model,
            stats: canonical_stats(hardware_stats),
            hardware_stats,
            last_record_image: None,
            last_policy_load_cycles: 0,
            last_observation_cycles: 0,
        })
    }

    pub fn reset(&mut self) -> Result<(), JitError> {
        self.model.reset().map_err(map_hj_error)?;
        self.last_record_image = None;
        self.last_policy_load_cycles = 0;
        self.last_observation_cycles = 0;
        self.refresh_stats()
    }

    pub fn load_policy(&mut self, policy: &VerifiedPolicy) -> Result<(), JitError> {
        self.last_record_image = None;
        self.last_policy_load_cycles = 0;
        self.last_observation_cycles = 0;
        let before = self.hardware_stats.cycles;
        if let Err(error) = self.model.load_policy(policy) {
            let load_error = map_hj_error(error);
            self.model.reset().map_err(map_hj_error)?;
            self.refresh_stats()?;
            return Err(load_error);
        }
        self.refresh_stats()?;
        self.last_policy_load_cycles =
            self.hardware_stats
                .cycles
                .checked_sub(before)
                .ok_or_else(|| {
                    JitError::new(
                        JitErrorCode::Backend,
                        "FPGA-Verilator cycle counter moved backwards during policy load",
                    )
                })?;
        Ok(())
    }

    pub fn observe(&mut self, event: &Event) -> Result<Decision, JitError> {
        self.last_record_image = None;
        self.last_observation_cycles = 0;
        let observation = match self.model.observe(event) {
            Ok(observation) => observation,
            Err(error) => {
                let _ = self.refresh_stats();
                return Err(map_hj_error(error));
            }
        };
        self.hardware_stats = observation.stats;
        self.stats = canonical_stats(observation.stats);
        self.last_record_image = observation.record_image;
        self.last_observation_cycles = observation.cycles;

        match (
            observation.accepted,
            observation.reject_code,
            observation.record,
        ) {
            (true, None, Some(record)) => Ok(Decision::Emit { record }),
            (true, None, None) => Ok(Decision::Accept),
            (false, Some(code), None) => Ok(Decision::Reject { code }),
            _ => Err(JitError::new(
                JitErrorCode::Backend,
                "FPGA-Verilator returned an inconsistent decision",
            )),
        }
    }

    pub fn stats(&self) -> Stats {
        self.stats
    }

    pub fn hardware_stats(&self) -> HjStats {
        self.hardware_stats
    }

    pub fn last_record_image(&self) -> Option<&HjRecordImage> {
        self.last_record_image.as_ref()
    }

    pub fn last_policy_load_cycles(&self) -> u64 {
        self.last_policy_load_cycles
    }

    pub fn last_observation_cycles(&self) -> u64 {
        self.last_observation_cycles
    }

    fn refresh_stats(&mut self) -> Result<(), JitError> {
        let stats = self.model.stats().map_err(map_hj_error)?;
        self.hardware_stats = stats;
        self.stats = canonical_stats(stats);
        Ok(())
    }
}

fn canonical_stats(stats: HjStats) -> Stats {
    Stats {
        event_count: stats.event_count,
        record_count: stats.record_count,
        metadata_bytes: stats.metadata_bytes,
        reject_count: stats.reject_count,
        drop_count: stats.drop_count,
        instruction_count: stats.instruction_count,
        epoch: stats.epoch,
    }
}

fn map_hj_error(error: HjError) -> JitError {
    let message = error.to_string();
    let code = match error {
        HjError::Construction | HjError::Protocol(_) | HjError::Decode(_) => JitErrorCode::Backend,
        HjError::Unsupported(_) | HjError::Policy(_) => JitErrorCode::Unsupported,
        HjError::Timeout => JitErrorCode::Timeout,
        HjError::Rtl(code) => rtl_error_code(code),
        HjError::Buffer => JitErrorCode::BudgetExceeded,
    };
    JitError::new(code, message)
}

fn rtl_error_code(code: u32) -> JitErrorCode {
    match code {
        1 => JitErrorCode::Null,
        2 => JitErrorCode::StructSize,
        3 => JitErrorCode::AbiVersion,
        4 => JitErrorCode::Parse,
        5 => JitErrorCode::PolicyVersion,
        6 => JitErrorCode::TooManyInstructions,
        7 => JitErrorCode::TooManyRanges,
        8 => JitErrorCode::InvalidRange,
        9 => JitErrorCode::InvalidStride,
        10 => JitErrorCode::BudgetExceeded,
        11 => JitErrorCode::InvalidControlFlow,
        12 => JitErrorCode::Unsupported,
        13 => JitErrorCode::DigestMismatch,
        14 => JitErrorCode::Rejected,
        15 => JitErrorCode::Drop,
        16 => JitErrorCode::Timeout,
        17 => JitErrorCode::Backend,
        18 => JitErrorCode::Io,
        19 => JitErrorCode::Poisoned,
        20 => JitErrorCode::Panic,
        _ => JitErrorCode::Backend,
    }
}
