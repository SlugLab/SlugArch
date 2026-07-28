use std::mem::MaybeUninit;

use slugarch_verilator_sys as sys;

const POLICY_BYTES: usize = sys::SLUGARCH_HJ_POLICY_BYTES as usize;
const EVENT_BYTES: usize = sys::SLUGARCH_HJ_EVENT_BYTES as usize;
const RECORD_BYTES: usize = sys::SLUGARCH_HJ_RECORD_BYTES as usize;

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn raw_validation_policy() -> [u8; POLICY_BYTES] {
    let mut image = [0; POLICY_BYTES];
    image[0..4].copy_from_slice(b"SJIT");
    put_u32(&mut image, 4, 1);
    put_u32(&mut image, 8, 1);
    put_u32(&mut image, 12, 1);
    put_u32(&mut image, 16, 4);
    put_u32(&mut image, 20, 1);
    put_u32(&mut image, 24, 256);
    put_u32(&mut image, 28, POLICY_BYTES as u32);
    image[32..64].fill(0xa5);

    // CAPTURE(validation), EMIT, EPOCH_FROM_PHASE, HALT.
    image[64] = 0x06;
    image[80] = 0x07;
    image[96] = 0x09;
    image[112] = 0x00;

    let range_offset = 64 + 32 * 16;
    image[range_offset..range_offset + 8].copy_from_slice(&(80_u64 * 1024 * 1024).to_le_bytes());
    image[range_offset + 8..range_offset + 16]
        .copy_from_slice(&(32_u64 * 1024 * 1024).to_le_bytes());
    image
}

fn raw_write_event() -> [u8; EVENT_BYTES] {
    let mut event = [0; EVENT_BYTES];
    event[0] = 2 << 4;
    event[1..3].copy_from_slice(&7_u16.to_le_bytes());
    event[3..11].copy_from_slice(&(80_u64 * 1024 * 1024).to_le_bytes());
    event[11..19].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    event[43..51].copy_from_slice(&42_u64.to_le_bytes());
    event[51..59].copy_from_slice(&3_u64.to_le_bytes());
    event[63] = 8;
    event
}

#[test]
fn raw_hj_abi_loads_observes_and_reports_exact_stats() {
    // SAFETY: every pointer below references a correctly sized live buffer,
    // and the opaque model is freed exactly once.
    unsafe {
        let hj = sys::slugarch_hj_new();
        assert!(!hj.is_null());
        sys::slugarch_hj_reset(hj);

        let image = raw_validation_policy();
        assert_eq!(
            sys::slugarch_hj_load_policy(hj, image.as_ptr(), (POLICY_BYTES - 1) as u32),
            sys::SLUGARCH_HJ_ERR_SIZE
        );
        assert_eq!(
            sys::slugarch_hj_load_policy(hj, image.as_ptr(), POLICY_BYTES as u32),
            sys::SLUGARCH_HJ_OK
        );

        let event = raw_write_event();
        let mut record = [0; RECORD_BYTES];
        let mut record_len = 0;
        let mut cycles = 0;
        assert_eq!(
            sys::slugarch_hj_observe(
                hj,
                event.as_ptr(),
                record.as_mut_ptr(),
                &mut record_len,
                &mut cycles,
            ),
            sys::SLUGARCH_HJ_OK
        );
        assert_eq!(record_len, 104);
        assert!(cycles > 0);
        assert_eq!(u64::from_le_bytes(record[4..12].try_into().unwrap()), 1);
        assert_eq!(u64::from_le_bytes(record[12..20].try_into().unwrap()), 42);
        assert_eq!(&record[20..52], &image[32..64]);
        assert_eq!(u64::from_le_bytes(record[52..60].try_into().unwrap()), 3);

        let mut stats = MaybeUninit::<sys::SlugarchHjStats>::uninit();
        assert_eq!(
            sys::slugarch_hj_stats(hj, stats.as_mut_ptr()),
            sys::SLUGARCH_HJ_OK
        );
        let stats = stats.assume_init();
        assert_eq!(stats.event_count, 1);
        assert_eq!(stats.record_count, 1);
        assert_eq!(stats.metadata_bytes, 8);
        assert_eq!(stats.reject_count, 0);
        assert_eq!(stats.drop_count, 0);
        assert_eq!(stats.policy_error, 0);
        assert_eq!(stats.policy_ready, 1);

        sys::slugarch_hj_free(hj);
    }
}

#[test]
fn raw_hj_abi_rejects_null_handles() {
    // SAFETY: the API explicitly defines null as a checked error boundary.
    unsafe {
        let mut stats = MaybeUninit::<sys::SlugarchHjStats>::uninit();
        assert_eq!(
            sys::slugarch_hj_stats(std::ptr::null(), stats.as_mut_ptr()),
            sys::SLUGARCH_HJ_ERR_NULL
        );
        sys::slugarch_hj_free(std::ptr::null_mut());
    }
}

#[test]
fn raw_hj_dense_delta_fails_closed_with_budget_error() {
    // SAFETY: every pointer references a correctly sized live buffer, and the
    // opaque model is freed exactly once.
    unsafe {
        let hj = sys::slugarch_hj_new();
        assert!(!hj.is_null());
        sys::slugarch_hj_reset(hj);

        let mut image = raw_validation_policy();
        image[65] = 1; // CAPTURE(delta).
        assert_eq!(
            sys::slugarch_hj_load_policy(hj, image.as_ptr(), POLICY_BYTES as u32),
            sys::SLUGARCH_HJ_OK
        );

        let mut event = raw_write_event();
        event[11..28].fill(1);
        event[63] = 17;
        let mut record = [0; RECORD_BYTES];
        let mut record_len = 0;
        let mut cycles = 0;
        assert_eq!(
            sys::slugarch_hj_observe(
                hj,
                event.as_ptr(),
                record.as_mut_ptr(),
                &mut record_len,
                &mut cycles,
            ),
            sys::SLUGARCH_HJ_ERR_RTL
        );
        assert_eq!(record_len, 0);
        assert!(cycles > 0);

        let mut stats = MaybeUninit::<sys::SlugarchHjStats>::uninit();
        assert_eq!(
            sys::slugarch_hj_stats(hj, stats.as_mut_ptr()),
            sys::SLUGARCH_HJ_OK
        );
        let stats = stats.assume_init();
        assert_eq!(stats.policy_error, 10);
        assert_eq!(stats.drop_count, 1);
        assert_eq!(stats.record_count, 0);
        assert_eq!(stats.policy_ready, 0);
        sys::slugarch_hj_free(hj);
    }
}
