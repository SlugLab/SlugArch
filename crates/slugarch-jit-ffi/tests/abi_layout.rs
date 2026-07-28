use std::mem::{align_of, size_of, MaybeUninit};
use std::ptr::{addr_of, null, null_mut};

use slugarch_jit_ffi::{
    slugarch_jit_abi_version, slugarch_jit_backend_caps, slugarch_jit_create, slugarch_jit_destroy,
    SlugJitCreateArgs, SlugJitDecision, SlugJitEvent, SlugJitHandle, SlugJitPolicyInfo,
    SlugJitStats, SLUG_JIT_ABI_VERSION, SLUG_JIT_BACKEND_RUST, SLUG_JIT_CAP_POLICY,
    SLUG_JIT_CAP_RECORD, SLUG_JIT_ERR_ABI_VERSION, SLUG_JIT_ERR_NULL, SLUG_JIT_ERR_STRUCT_SIZE,
    SLUG_JIT_OK,
};

macro_rules! offset_of {
    ($ty:ty, $field:ident) => {{
        let uninit = MaybeUninit::<$ty>::uninit();
        let base = uninit.as_ptr();
        // SAFETY: addr_of! forms a field pointer without reading uninitialized memory.
        unsafe { addr_of!((*base).$field) as usize - base as usize }
    }};
}

fn create_args() -> SlugJitCreateArgs {
    SlugJitCreateArgs {
        struct_size: size_of::<SlugJitCreateArgs>() as u32,
        abi_version: SLUG_JIT_ABI_VERSION,
        backend: SLUG_JIT_BACKEND_RUST,
        strict: 1,
        diagnostic_capacity: 256,
        reserved: 0,
    }
}

#[test]
fn abi_one_layout_matches_the_checked_in_c_prefix() {
    assert_eq!(slugarch_jit_abi_version(), 1);
    assert_eq!(SLUG_JIT_ABI_VERSION, 1);
    assert_eq!(
        slugarch_jit_backend_caps(),
        SLUG_JIT_CAP_POLICY | SLUG_JIT_CAP_RECORD
    );

    assert_eq!(
        (
            size_of::<SlugJitCreateArgs>(),
            align_of::<SlugJitCreateArgs>()
        ),
        (24, 4)
    );
    assert_eq!(offset_of!(SlugJitCreateArgs, struct_size), 0);
    assert_eq!(offset_of!(SlugJitCreateArgs, abi_version), 4);
    assert_eq!(offset_of!(SlugJitCreateArgs, backend), 8);
    assert_eq!(offset_of!(SlugJitCreateArgs, strict), 12);
    assert_eq!(offset_of!(SlugJitCreateArgs, diagnostic_capacity), 16);
    assert_eq!(offset_of!(SlugJitCreateArgs, reserved), 20);

    assert_eq!(
        (size_of::<SlugJitEvent>(), align_of::<SlugJitEvent>()),
        (144, 8)
    );
    assert_eq!(offset_of!(SlugJitEvent, struct_size), 0);
    assert_eq!(offset_of!(SlugJitEvent, abi_version), 4);
    assert_eq!(offset_of!(SlugJitEvent, event_id), 8);
    assert_eq!(offset_of!(SlugJitEvent, client_id), 16);
    assert_eq!(offset_of!(SlugJitEvent, direction), 24);
    assert_eq!(offset_of!(SlugJitEvent, event_class), 28);
    assert_eq!(offset_of!(SlugJitEvent, opcode), 32);
    assert_eq!(offset_of!(SlugJitEvent, payload_len), 36);
    assert_eq!(offset_of!(SlugJitEvent, address), 40);
    assert_eq!(offset_of!(SlugJitEvent, tag), 48);
    assert_eq!(offset_of!(SlugJitEvent, phase_id), 56);
    assert_eq!(offset_of!(SlugJitEvent, monotonic_ns), 64);
    assert_eq!(offset_of!(SlugJitEvent, status), 72);
    assert_eq!(offset_of!(SlugJitEvent, reserved), 76);
    assert_eq!(offset_of!(SlugJitEvent, payload), 80);

    assert_eq!(
        (
            size_of::<SlugJitPolicyInfo>(),
            align_of::<SlugJitPolicyInfo>()
        ),
        (64, 4)
    );
    assert_eq!(offset_of!(SlugJitPolicyInfo, digest), 16);
    assert_eq!(offset_of!(SlugJitPolicyInfo, instruction_count), 48);
    assert_eq!(offset_of!(SlugJitPolicyInfo, range_count), 52);
    assert_eq!(offset_of!(SlugJitPolicyInfo, metadata_budget), 56);

    assert_eq!(
        (size_of::<SlugJitDecision>(), align_of::<SlugJitDecision>()),
        (48, 8)
    );
    assert_eq!(offset_of!(SlugJitDecision, epoch), 32);
    assert_eq!(offset_of!(SlugJitDecision, record_id), 40);

    assert_eq!(
        (size_of::<SlugJitStats>(), align_of::<SlugJitStats>()),
        (56, 8)
    );
    assert_eq!(offset_of!(SlugJitStats, event_count), 8);
    assert_eq!(offset_of!(SlugJitStats, epoch), 48);
}

#[test]
fn create_rejects_null_short_and_wrong_version_prefixes() {
    let mut handle: *mut SlugJitHandle = null_mut();
    let mut args = create_args();

    // SAFETY: each pointer is either valid for its declared prefix or deliberately null.
    unsafe {
        assert_eq!(slugarch_jit_create(null(), &mut handle), SLUG_JIT_ERR_NULL);
        assert_eq!(slugarch_jit_create(&args, null_mut()), SLUG_JIT_ERR_NULL);

        args.struct_size = (size_of::<SlugJitCreateArgs>() - 1) as u32;
        assert_eq!(
            slugarch_jit_create(&args, &mut handle),
            SLUG_JIT_ERR_STRUCT_SIZE
        );

        args = create_args();
        args.abi_version += 1;
        assert_eq!(
            slugarch_jit_create(&args, &mut handle),
            SLUG_JIT_ERR_ABI_VERSION
        );

        slugarch_jit_destroy(null_mut());
    }
    assert!(handle.is_null());
}

#[test]
fn create_accepts_an_oversized_known_prefix() {
    #[repr(C)]
    struct ExtendedCreateArgs {
        prefix: SlugJitCreateArgs,
        unknown_tail: [u8; 16],
    }

    let mut extended = ExtendedCreateArgs {
        prefix: create_args(),
        unknown_tail: [0xa5; 16],
    };
    extended.prefix.struct_size = size_of::<ExtendedCreateArgs>() as u32;
    let mut handle: *mut SlugJitHandle = null_mut();

    // SAFETY: the prefix has the declared minimum layout and remains live for the call.
    unsafe {
        assert_eq!(
            slugarch_jit_create(&extended.prefix, &mut handle),
            SLUG_JIT_OK
        );
        assert!(!handle.is_null());
        slugarch_jit_destroy(handle);
    }
}
