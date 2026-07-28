use std::mem::size_of;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr::{null, null_mut};

use slugarch_jit_ffi::{
    slugarch_jit_create, slugarch_jit_destroy, slugarch_jit_last_diagnostic,
    slugarch_jit_load_policy, SlugJitCreateArgs, SlugJitHandle, SlugJitPolicyInfo,
    SLUG_JIT_ABI_VERSION, SLUG_JIT_BACKEND_RUST, SLUG_JIT_ERR_NULL, SLUG_JIT_ERR_PARSE,
    SLUG_JIT_OK,
};

fn create_handle() -> *mut SlugJitHandle {
    let args = SlugJitCreateArgs {
        struct_size: size_of::<SlugJitCreateArgs>() as u32,
        abi_version: SLUG_JIT_ABI_VERSION,
        backend: SLUG_JIT_BACKEND_RUST,
        strict: 1,
        diagnostic_capacity: 256,
        reserved: 0,
    };
    let mut handle = null_mut();
    // SAFETY: args and out pointer are live for the call.
    assert_eq!(
        unsafe { slugarch_jit_create(&args, &mut handle) },
        SLUG_JIT_OK
    );
    handle
}

fn policy_info() -> SlugJitPolicyInfo {
    SlugJitPolicyInfo {
        struct_size: size_of::<SlugJitPolicyInfo>() as u32,
        abi_version: SLUG_JIT_ABI_VERSION,
        backend: 0,
        canonical_bytes: 0,
        digest: [0; 32],
        instruction_count: 0,
        range_count: 0,
        metadata_budget: 0,
        reserved: 0,
    }
}

#[test]
fn malformed_inputs_return_stable_errors_without_unwinding() {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let handle = create_handle();
        let mut info = policy_info();

        // SAFETY: pointers are deliberately null or point to live objects.
        unsafe {
            assert_eq!(
                slugarch_jit_load_policy(handle, null(), 1, &mut info),
                SLUG_JIT_ERR_NULL
            );
            assert_eq!(
                slugarch_jit_load_policy(null_mut(), b"{" as *const u8, 1, &mut info),
                SLUG_JIT_ERR_NULL
            );
            assert_eq!(
                slugarch_jit_last_diagnostic(handle, null_mut(), 0, null_mut()),
                SLUG_JIT_ERR_NULL
            );
            slugarch_jit_destroy(handle);
        }
    }));

    assert!(outcome.is_ok(), "an unwind crossed the exported ABI");
}

#[test]
fn diagnostic_length_and_bytes_are_idempotent() {
    let handle = create_handle();
    let mut info = policy_info();
    let bad_policy = b"{";

    // SAFETY: all pointers refer to live storage for the duration of each call.
    unsafe {
        assert_eq!(
            slugarch_jit_load_policy(
                handle,
                bad_policy.as_ptr(),
                bad_policy.len() as u32,
                &mut info,
            ),
            SLUG_JIT_ERR_PARSE
        );

        let mut first_len = 0;
        let mut second_len = 0;
        assert_eq!(
            slugarch_jit_last_diagnostic(handle, null_mut(), 0, &mut first_len),
            SLUG_JIT_OK
        );
        assert_eq!(
            slugarch_jit_last_diagnostic(handle, null_mut(), 0, &mut second_len),
            SLUG_JIT_OK
        );
        assert_eq!(first_len, second_len);
        assert!(first_len > 0);

        let mut first = vec![0; first_len as usize];
        let mut second = vec![0; second_len as usize];
        let mut first_written = 0;
        let mut second_written = 0;
        assert_eq!(
            slugarch_jit_last_diagnostic(
                handle,
                first.as_mut_ptr(),
                first.len() as u32,
                &mut first_written,
            ),
            SLUG_JIT_OK
        );
        assert_eq!(
            slugarch_jit_last_diagnostic(
                handle,
                second.as_mut_ptr(),
                second.len() as u32,
                &mut second_written,
            ),
            SLUG_JIT_OK
        );
        assert_eq!(first_written, second_written);
        assert_eq!(first, second);

        slugarch_jit_destroy(handle);
    }
}
