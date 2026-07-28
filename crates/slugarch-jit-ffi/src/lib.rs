use std::mem::size_of;
use std::ptr::{self, null_mut};
use std::slice;
use std::sync::{Mutex, MutexGuard};

use slugarch_jit::{
    Decision, Direction, Engine, Event, EventClass, JitError, JitErrorCode, Policy, Stats,
    VerifiedPolicy, MAX_EVENT_PAYLOAD,
};

#[cfg(feature = "fpga-verilator")]
pub mod fpga;

pub const SLUG_JIT_ABI_VERSION: u32 = slugarch_jit::SLUG_JIT_ABI_VERSION;
pub const SLUG_JIT_PAYLOAD_BYTES: usize = MAX_EVENT_PAYLOAD;
pub const SLUG_JIT_DIGEST_BYTES: usize = 32;

pub const SLUG_JIT_BACKEND_NONE: u32 = 0;
pub const SLUG_JIT_BACKEND_RUST: u32 = 1;
pub const SLUG_JIT_BACKEND_GPU: u32 = 2;
pub const SLUG_JIT_BACKEND_FPGA_VERILATOR: u32 = 3;

pub const SLUG_JIT_CAP_POLICY: u64 = 1 << 0;
pub const SLUG_JIT_CAP_RECORD: u64 = 1 << 1;
pub const SLUG_JIT_CAP_GPU_DIAGNOSTIC: u64 = 1 << 2;
pub const SLUG_JIT_CAP_FPGA_RTL: u64 = 1 << 3;

pub const SLUG_JIT_OK: i32 = 0;
pub const SLUG_JIT_ERR_NULL: i32 = JitErrorCode::Null as i32;
pub const SLUG_JIT_ERR_STRUCT_SIZE: i32 = JitErrorCode::StructSize as i32;
pub const SLUG_JIT_ERR_ABI_VERSION: i32 = JitErrorCode::AbiVersion as i32;
pub const SLUG_JIT_ERR_PARSE: i32 = JitErrorCode::Parse as i32;
pub const SLUG_JIT_ERR_POLICY_VERSION: i32 = JitErrorCode::PolicyVersion as i32;
pub const SLUG_JIT_ERR_TOO_MANY_INSTRUCTIONS: i32 = JitErrorCode::TooManyInstructions as i32;
pub const SLUG_JIT_ERR_TOO_MANY_RANGES: i32 = JitErrorCode::TooManyRanges as i32;
pub const SLUG_JIT_ERR_INVALID_RANGE: i32 = JitErrorCode::InvalidRange as i32;
pub const SLUG_JIT_ERR_INVALID_STRIDE: i32 = JitErrorCode::InvalidStride as i32;
pub const SLUG_JIT_ERR_BUDGET_EXCEEDED: i32 = JitErrorCode::BudgetExceeded as i32;
pub const SLUG_JIT_ERR_INVALID_CONTROL_FLOW: i32 = JitErrorCode::InvalidControlFlow as i32;
pub const SLUG_JIT_ERR_UNSUPPORTED: i32 = JitErrorCode::Unsupported as i32;
pub const SLUG_JIT_ERR_DIGEST_MISMATCH: i32 = JitErrorCode::DigestMismatch as i32;
pub const SLUG_JIT_ERR_REJECTED: i32 = JitErrorCode::Rejected as i32;
pub const SLUG_JIT_ERR_DROP: i32 = JitErrorCode::Drop as i32;
pub const SLUG_JIT_ERR_TIMEOUT: i32 = JitErrorCode::Timeout as i32;
pub const SLUG_JIT_ERR_BACKEND: i32 = JitErrorCode::Backend as i32;
pub const SLUG_JIT_ERR_IO: i32 = JitErrorCode::Io as i32;
pub const SLUG_JIT_ERR_POISONED: i32 = JitErrorCode::Poisoned as i32;
pub const SLUG_JIT_ERR_PANIC: i32 = JitErrorCode::Panic as i32;

const MAX_DIAGNOSTIC_BYTES: u32 = 64 * 1024;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SlugJitCreateArgs {
    pub struct_size: u32,
    pub abi_version: u32,
    pub backend: u32,
    pub strict: u32,
    pub diagnostic_capacity: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SlugJitEvent {
    pub struct_size: u32,
    pub abi_version: u32,
    pub event_id: u64,
    pub client_id: u64,
    pub direction: u32,
    pub event_class: u32,
    pub opcode: u32,
    pub payload_len: u32,
    pub address: u64,
    pub tag: u64,
    pub phase_id: u64,
    pub monotonic_ns: u64,
    pub status: u32,
    pub reserved: u32,
    pub payload: [u8; SLUG_JIT_PAYLOAD_BYTES],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SlugJitPolicyInfo {
    pub struct_size: u32,
    pub abi_version: u32,
    pub backend: u32,
    pub canonical_bytes: u32,
    pub digest: [u8; SLUG_JIT_DIGEST_BYTES],
    pub instruction_count: u32,
    pub range_count: u32,
    pub metadata_budget: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SlugJitDecision {
    pub struct_size: u32,
    pub abi_version: u32,
    pub accepted: u32,
    pub emitted: u32,
    pub error_code: u32,
    pub record_bytes: u32,
    pub payload_bytes: u32,
    pub reserved: u32,
    pub epoch: u64,
    pub record_id: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SlugJitStats {
    pub struct_size: u32,
    pub abi_version: u32,
    pub event_count: u64,
    pub record_count: u64,
    pub metadata_bytes: u64,
    pub reject_count: u64,
    pub drop_count: u64,
    pub epoch: u64,
}

enum BackendState {
    Rust(Option<Engine>),
    #[cfg(feature = "fpga-verilator")]
    Fpga(fpga::FpgaBackend),
}

impl BackendState {
    fn create(backend: u32) -> Result<Self, JitError> {
        match backend {
            SLUG_JIT_BACKEND_RUST => Ok(Self::Rust(None)),
            SLUG_JIT_BACKEND_FPGA_VERILATOR => {
                #[cfg(feature = "fpga-verilator")]
                {
                    fpga::FpgaBackend::new().map(Self::Fpga)
                }
                #[cfg(not(feature = "fpga-verilator"))]
                {
                    Err(error(
                        JitErrorCode::Unsupported,
                        "the selected FPGA-Verilator backend is not compiled in",
                    ))
                }
            }
            _ => Err(error(
                JitErrorCode::Unsupported,
                "the selected JIT backend is unavailable",
            )),
        }
    }

    fn id(&self) -> u32 {
        match self {
            Self::Rust(_) => SLUG_JIT_BACKEND_RUST,
            #[cfg(feature = "fpga-verilator")]
            Self::Fpga(_) => SLUG_JIT_BACKEND_FPGA_VERILATOR,
        }
    }

    fn clear_policy(&mut self) -> Result<(), JitError> {
        match self {
            Self::Rust(engine) => {
                *engine = None;
                Ok(())
            }
            #[cfg(feature = "fpga-verilator")]
            Self::Fpga(engine) => engine.reset(),
        }
    }

    fn load_policy(&mut self, policy: VerifiedPolicy) -> Result<(), JitError> {
        match self {
            Self::Rust(engine) => {
                *engine = Some(Engine::new(policy));
                Ok(())
            }
            #[cfg(feature = "fpga-verilator")]
            Self::Fpga(engine) => engine.load_policy(&policy),
        }
    }

    fn observe(&mut self, event: &Event) -> Result<Decision, JitError> {
        match self {
            Self::Rust(engine) => engine
                .as_mut()
                .ok_or_else(|| error(JitErrorCode::Backend, "no policy is installed"))?
                .observe(event),
            #[cfg(feature = "fpga-verilator")]
            Self::Fpga(engine) => engine.observe(event),
        }
    }

    fn stats(&self) -> Stats {
        match self {
            Self::Rust(engine) => engine
                .as_ref()
                .map(Engine::stats)
                .unwrap_or_else(Stats::default),
            #[cfg(feature = "fpga-verilator")]
            Self::Fpga(engine) => engine.stats(),
        }
    }
}

struct EngineState {
    backend: BackendState,
    diagnostic_capacity: usize,
    diagnostic: Vec<u8>,
}

impl EngineState {
    fn set_diagnostic(&mut self, message: &str) {
        self.diagnostic.clear();
        let bytes = message.as_bytes();
        let length = bytes.len().min(self.diagnostic_capacity);
        self.diagnostic.extend_from_slice(&bytes[..length]);
    }

    fn clear_diagnostic(&mut self) {
        self.diagnostic.clear();
    }
}

#[repr(C)]
pub struct SlugJitHandle {
    state: Mutex<EngineState>,
}

fn error(code: JitErrorCode, message: impl Into<String>) -> JitError {
    JitError::new(code, message)
}

fn ffi_guard<F>(operation: F) -> i32
where
    F: FnOnce() -> Result<(), JitError>,
{
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)) {
        Ok(Ok(())) => SLUG_JIT_OK,
        Ok(Err(error)) => error.code() as i32,
        Err(_) => SLUG_JIT_ERR_PANIC,
    }
}

unsafe fn validate_prefix<T>(value: *const T) -> Result<u32, JitError> {
    if value.is_null() {
        return Err(error(JitErrorCode::Null, "structure pointer is null"));
    }

    let words = value.cast::<u32>();
    // SAFETY: the ABI requires every structure pointer to expose two u32 prefix words.
    let struct_size = unsafe { ptr::read_unaligned(words) };
    // SAFETY: the ABI requires every structure pointer to expose two u32 prefix words.
    let abi_version = unsafe { ptr::read_unaligned(words.add(1)) };

    if struct_size < size_of::<T>() as u32 {
        return Err(error(
            JitErrorCode::StructSize,
            "structure is smaller than the ABI 1 prefix",
        ));
    }
    if abi_version != SLUG_JIT_ABI_VERSION {
        return Err(error(
            JitErrorCode::AbiVersion,
            "structure ABI version is not 1",
        ));
    }
    Ok(struct_size)
}

unsafe fn lock_handle<'a>(
    handle: *mut SlugJitHandle,
) -> Result<MutexGuard<'a, EngineState>, JitError> {
    // SAFETY: the caller promises a live handle allocated by slugarch_jit_create.
    let handle = unsafe { handle.as_ref() }
        .ok_or_else(|| error(JitErrorCode::Null, "JIT handle is null"))?;
    handle
        .state
        .lock()
        .map_err(|_| error(JitErrorCode::Poisoned, "JIT handle lock is poisoned"))
}

fn checked_u32(value: usize, field: &str) -> Result<u32, JitError> {
    u32::try_from(value).map_err(|_| error(JitErrorCode::Backend, format!("{field} exceeds u32")))
}

fn event_direction(value: u32) -> Result<Direction, JitError> {
    match value {
        0 => Ok(Direction::HostToDevice),
        1 => Ok(Direction::DeviceToHost),
        _ => Err(error(
            JitErrorCode::Unsupported,
            "event direction is unsupported",
        )),
    }
}

fn event_class(value: u32) -> Result<EventClass, JitError> {
    match value {
        1 => Ok(EventClass::CxlMemRead),
        2 => Ok(EventClass::CxlMemWrite),
        3 => Ok(EventClass::CxlMemData),
        4 => Ok(EventClass::Completion),
        5 => Ok(EventClass::PtxModuleLoad),
        6 => Ok(EventClass::KernelLaunch),
        7 => Ok(EventClass::Phase),
        8 => Ok(EventClass::Fence),
        _ => Err(error(
            JitErrorCode::Unsupported,
            "event class is unsupported",
        )),
    }
}

#[no_mangle]
pub extern "C" fn slugarch_jit_abi_version() -> u32 {
    SLUG_JIT_ABI_VERSION
}

#[no_mangle]
pub extern "C" fn slugarch_jit_backend_caps() -> u64 {
    let capabilities = SLUG_JIT_CAP_POLICY | SLUG_JIT_CAP_RECORD;
    #[cfg(feature = "fpga-verilator")]
    {
        capabilities | SLUG_JIT_CAP_FPGA_RTL
    }
    #[cfg(not(feature = "fpga-verilator"))]
    {
        capabilities
    }
}

/// Creates one isolated Rust policy engine.
///
/// # Safety
///
/// `args` must expose at least its declared ABI prefix and `out` must be
/// writable. The returned handle must be destroyed exactly once.
#[no_mangle]
pub unsafe extern "C" fn slugarch_jit_create(
    args: *const SlugJitCreateArgs,
    out: *mut *mut SlugJitHandle,
) -> i32 {
    ffi_guard(|| {
        if out.is_null() {
            return Err(error(JitErrorCode::Null, "create output is null"));
        }
        // SAFETY: out is nonnull and writable by the caller.
        unsafe { ptr::write(out, null_mut()) };
        // SAFETY: validate_prefix checks the two-word ABI prefix before full access.
        unsafe { validate_prefix(args)? };
        // SAFETY: the validated prefix is at least sizeof(SlugJitCreateArgs).
        let args = unsafe { ptr::read(args) };

        if args.strict != 1 || args.reserved != 0 {
            return Err(error(
                JitErrorCode::Unsupported,
                "only strict backend selection is available",
            ));
        }
        if args.diagnostic_capacity > MAX_DIAGNOSTIC_BYTES {
            return Err(error(
                JitErrorCode::BudgetExceeded,
                "diagnostic capacity exceeds 64 KiB",
            ));
        }

        let backend = BackendState::create(args.backend)?;
        let handle = Box::new(SlugJitHandle {
            state: Mutex::new(EngineState {
                backend,
                diagnostic_capacity: args.diagnostic_capacity as usize,
                diagnostic: Vec::new(),
            }),
        });
        // SAFETY: out is nonnull and receives ownership of the boxed handle.
        unsafe { ptr::write(out, Box::into_raw(handle)) };
        Ok(())
    })
}

/// Parses, verifies, and installs one policy into `handle`.
///
/// # Safety
///
/// All pointers must remain live for the call. `json` must expose `json_len`
/// bytes and `out` must expose its declared ABI prefix.
#[no_mangle]
pub unsafe extern "C" fn slugarch_jit_load_policy(
    handle: *mut SlugJitHandle,
    json: *const u8,
    json_len: u32,
    out: *mut SlugJitPolicyInfo,
) -> i32 {
    ffi_guard(|| {
        if json.is_null() || out.is_null() {
            return Err(error(JitErrorCode::Null, "policy input or output is null"));
        }
        // SAFETY: validate_prefix checks the two-word ABI prefix before full access.
        unsafe { validate_prefix(out)? };
        // SAFETY: handle must be live by the ABI contract.
        let mut state = unsafe { lock_handle(handle)? };
        // SAFETY: json points to json_len caller-owned bytes for this call.
        let json = unsafe { slice::from_raw_parts(json, json_len as usize) };

        let verified = match Policy::parse(json).and_then(|policy| policy.verify()) {
            Ok(verified) => verified,
            Err(parse_error) => {
                let _ = state.backend.clear_policy();
                state.set_diagnostic(&parse_error.to_string());
                return Err(parse_error);
            }
        };
        let canonical_bytes = checked_u32(verified.canonical_json.len(), "canonical policy")?;
        let instruction_count = checked_u32(verified.instructions.len(), "instruction count")?;
        let range_count = checked_u32(verified.ranges.len(), "range count")?;
        let info = SlugJitPolicyInfo {
            struct_size: size_of::<SlugJitPolicyInfo>() as u32,
            abi_version: SLUG_JIT_ABI_VERSION,
            backend: state.backend.id(),
            canonical_bytes,
            digest: verified.digest,
            instruction_count,
            range_count,
            metadata_budget: verified.metadata_budget,
            reserved: 0,
        };

        if let Err(load_error) = state.backend.load_policy(verified) {
            let _ = state.backend.clear_policy();
            state.set_diagnostic(&load_error.to_string());
            return Err(load_error);
        }
        state.clear_diagnostic();
        // SAFETY: out was validated as a writable ABI 1 prefix.
        unsafe { ptr::write(out, info) };
        Ok(())
    })
}

/// Observes one fixed-size event with the policy installed in `handle`.
///
/// # Safety
///
/// `handle` must be live and `event`/`out` must expose their declared ABI
/// prefixes for the duration of the call.
#[no_mangle]
pub unsafe extern "C" fn slugarch_jit_observe(
    handle: *mut SlugJitHandle,
    event: *const SlugJitEvent,
    out: *mut SlugJitDecision,
) -> i32 {
    ffi_guard(|| {
        if out.is_null() {
            return Err(error(JitErrorCode::Null, "decision output is null"));
        }
        // SAFETY: each helper checks the two-word ABI prefix before full access.
        unsafe {
            validate_prefix(event)?;
            validate_prefix(out)?;
        }
        // SAFETY: the event prefix is at least sizeof(SlugJitEvent).
        let raw = unsafe { ptr::read(event) };
        if raw.reserved != 0
            || raw.opcode > u32::from(u16::MAX)
            || raw.payload_len > MAX_EVENT_PAYLOAD as u32
        {
            return Err(error(
                JitErrorCode::Unsupported,
                "event contains an unsupported field value",
            ));
        }
        let event = Event {
            event_id: raw.event_id,
            client_id: raw.client_id,
            direction: event_direction(raw.direction)?,
            class: event_class(raw.event_class)?,
            opcode: raw.opcode as u16,
            address: raw.address,
            payload_len: raw.payload_len as u8,
            payload: raw.payload,
            tag: raw.tag,
            phase_id: raw.phase_id,
            monotonic_ns: raw.monotonic_ns,
            status: raw.status,
        };

        // SAFETY: handle must be live by the ABI contract.
        let mut state = unsafe { lock_handle(handle)? };
        let decision = match state.backend.observe(&event) {
            Ok(decision) => decision,
            Err(observe_error) => {
                state.set_diagnostic(&observe_error.to_string());
                return Err(observe_error);
            }
        };
        let stats = state.backend.stats();
        let (accepted, emitted, error_code, record_bytes, payload_bytes, epoch, record_id) =
            match decision {
                Decision::Accept => (1, 0, 0, 0, 0, stats.epoch, 0),
                Decision::Emit { record } => (
                    1,
                    1,
                    0,
                    record.encoded_len(),
                    u32::try_from(record.payload.captured_bytes()).map_err(|_| {
                        error(JitErrorCode::Backend, "record payload length exceeds u32")
                    })?,
                    record.epoch,
                    record.sequence,
                ),
                Decision::Reject { .. } => {
                    (0, 0, SLUG_JIT_ERR_REJECTED as u32, 0, 0, stats.epoch, 0)
                }
            };
        let decision = SlugJitDecision {
            struct_size: size_of::<SlugJitDecision>() as u32,
            abi_version: SLUG_JIT_ABI_VERSION,
            accepted,
            emitted,
            error_code,
            record_bytes,
            payload_bytes,
            reserved: 0,
            epoch,
            record_id,
        };
        state.clear_diagnostic();
        // SAFETY: out was validated as a writable ABI 1 prefix.
        unsafe { ptr::write(out, decision) };
        Ok(())
    })
}

/// Reads monotonically accumulated engine statistics.
///
/// # Safety
///
/// `handle` must be live and `out` must expose its declared writable ABI
/// prefix for the duration of the call.
#[no_mangle]
pub unsafe extern "C" fn slugarch_jit_stats(
    handle: *mut SlugJitHandle,
    out: *mut SlugJitStats,
) -> i32 {
    ffi_guard(|| {
        if out.is_null() {
            return Err(error(JitErrorCode::Null, "stats output is null"));
        }
        // SAFETY: validate_prefix checks the two-word ABI prefix before full access.
        unsafe { validate_prefix(out)? };
        // SAFETY: handle must be live by the ABI contract.
        let state = unsafe { lock_handle(handle)? };
        let stats = state.backend.stats();
        let stats = SlugJitStats {
            struct_size: size_of::<SlugJitStats>() as u32,
            abi_version: SLUG_JIT_ABI_VERSION,
            event_count: stats.event_count,
            record_count: stats.record_count,
            metadata_bytes: stats.metadata_bytes,
            reject_count: stats.reject_count,
            drop_count: stats.drop_count,
            epoch: stats.epoch,
        };
        // SAFETY: out was validated as a writable ABI 1 prefix.
        unsafe { ptr::write(out, stats) };
        Ok(())
    })
}

/// Copies the last bounded diagnostic without consuming it.
///
/// Passing `out == NULL` with `capacity == 0` is a length query.
///
/// # Safety
///
/// `handle` and `written` must be live. A nonnull `out` must expose `capacity`
/// writable bytes.
#[no_mangle]
pub unsafe extern "C" fn slugarch_jit_last_diagnostic(
    handle: *mut SlugJitHandle,
    out: *mut u8,
    capacity: u32,
    written: *mut u32,
) -> i32 {
    ffi_guard(|| {
        if written.is_null() || (out.is_null() && capacity != 0) {
            return Err(error(
                JitErrorCode::Null,
                "diagnostic output or length is null",
            ));
        }
        // SAFETY: handle must be live by the ABI contract.
        let state = unsafe { lock_handle(handle)? };
        let length = checked_u32(state.diagnostic.len(), "diagnostic length")?;
        // SAFETY: written is nonnull and writable by the caller.
        unsafe { ptr::write(written, length) };

        if out.is_null() {
            return Ok(());
        }
        if capacity < length {
            return Err(error(
                JitErrorCode::StructSize,
                "diagnostic output capacity is too small",
            ));
        }
        // SAFETY: out exposes capacity bytes and length <= capacity.
        unsafe { ptr::copy_nonoverlapping(state.diagnostic.as_ptr(), out, length as usize) };
        Ok(())
    })
}

/// Destroys a handle returned by `slugarch_jit_create`; null is a no-op.
///
/// # Safety
///
/// A nonnull handle must be live and must not be used or destroyed again.
#[no_mangle]
pub unsafe extern "C" fn slugarch_jit_destroy(handle: *mut SlugJitHandle) {
    if handle.is_null() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: the caller transfers the unique box ownership back exactly once.
        drop(unsafe { Box::from_raw(handle) });
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_guard_maps_panics_to_the_stable_code() {
        assert_eq!(
            ffi_guard(|| -> Result<(), JitError> { panic!("contained") }),
            SLUG_JIT_ERR_PANIC
        );
    }
}
