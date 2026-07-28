#ifndef SLUGARCH_JIT_H
#define SLUGARCH_JIT_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define SLUG_JIT_ABI_VERSION 1u
#define SLUG_JIT_PAYLOAD_BYTES 64u
#define SLUG_JIT_DIGEST_BYTES 32u

typedef struct SlugJitHandle SlugJitHandle;

enum {
    SLUG_JIT_BACKEND_NONE = 0,
    SLUG_JIT_BACKEND_RUST = 1,
    SLUG_JIT_BACKEND_GPU = 2,
    SLUG_JIT_BACKEND_FPGA_VERILATOR = 3,
};

enum {
    SLUG_JIT_CAP_POLICY = UINT64_C(1) << 0,
    SLUG_JIT_CAP_RECORD = UINT64_C(1) << 1,
    SLUG_JIT_CAP_GPU_DIAGNOSTIC = UINT64_C(1) << 2,
    SLUG_JIT_CAP_FPGA_RTL = UINT64_C(1) << 3,
};

enum {
    SLUG_JIT_OK = 0,
    SLUG_JIT_ERR_NULL = 1,
    SLUG_JIT_ERR_STRUCT_SIZE = 2,
    SLUG_JIT_ERR_ABI_VERSION = 3,
    SLUG_JIT_ERR_PARSE = 4,
    SLUG_JIT_ERR_POLICY_VERSION = 5,
    SLUG_JIT_ERR_TOO_MANY_INSTRUCTIONS = 6,
    SLUG_JIT_ERR_TOO_MANY_RANGES = 7,
    SLUG_JIT_ERR_INVALID_RANGE = 8,
    SLUG_JIT_ERR_INVALID_STRIDE = 9,
    SLUG_JIT_ERR_BUDGET_EXCEEDED = 10,
    SLUG_JIT_ERR_INVALID_CONTROL_FLOW = 11,
    SLUG_JIT_ERR_UNSUPPORTED = 12,
    SLUG_JIT_ERR_DIGEST_MISMATCH = 13,
    SLUG_JIT_ERR_REJECTED = 14,
    SLUG_JIT_ERR_DROP = 15,
    SLUG_JIT_ERR_TIMEOUT = 16,
    SLUG_JIT_ERR_BACKEND = 17,
    SLUG_JIT_ERR_IO = 18,
    SLUG_JIT_ERR_POISONED = 19,
    SLUG_JIT_ERR_PANIC = 20,
};

typedef struct SlugJitCreateArgs {
    uint32_t struct_size;
    uint32_t abi_version;
    uint32_t backend;
    uint32_t strict;
    uint32_t diagnostic_capacity;
    uint32_t reserved;
} SlugJitCreateArgs;

typedef struct SlugJitEvent {
    uint32_t struct_size;
    uint32_t abi_version;
    uint64_t event_id;
    uint64_t client_id;
    uint32_t direction;
    uint32_t event_class;
    uint32_t opcode;
    uint32_t payload_len;
    uint64_t address;
    uint64_t tag;
    uint64_t phase_id;
    uint64_t monotonic_ns;
    uint32_t status;
    uint32_t reserved;
    uint8_t payload[SLUG_JIT_PAYLOAD_BYTES];
} SlugJitEvent;

typedef struct SlugJitPolicyInfo {
    uint32_t struct_size;
    uint32_t abi_version;
    uint32_t backend;
    uint32_t canonical_bytes;
    uint8_t digest[SLUG_JIT_DIGEST_BYTES];
    uint32_t instruction_count;
    uint32_t range_count;
    uint32_t metadata_budget;
    uint32_t reserved;
} SlugJitPolicyInfo;

typedef struct SlugJitDecision {
    uint32_t struct_size;
    uint32_t abi_version;
    uint32_t accepted;
    uint32_t emitted;
    uint32_t error_code;
    uint32_t record_bytes;
    uint32_t payload_bytes;
    uint32_t reserved;
    uint64_t epoch;
    uint64_t record_id;
} SlugJitDecision;

typedef struct SlugJitStats {
    uint32_t struct_size;
    uint32_t abi_version;
    uint64_t event_count;
    uint64_t record_count;
    uint64_t metadata_bytes;
    uint64_t reject_count;
    uint64_t drop_count;
    uint64_t epoch;
} SlugJitStats;

uint32_t slugarch_jit_abi_version(void);
uint64_t slugarch_jit_backend_caps(void);
int32_t slugarch_jit_create(const SlugJitCreateArgs *args,
                            SlugJitHandle **out);
int32_t slugarch_jit_load_policy(SlugJitHandle *handle,
                                 const uint8_t *json, uint32_t json_len,
                                 SlugJitPolicyInfo *out);
int32_t slugarch_jit_observe(SlugJitHandle *handle,
                             const SlugJitEvent *event,
                             SlugJitDecision *out);
int32_t slugarch_jit_stats(SlugJitHandle *handle, SlugJitStats *out);
int32_t slugarch_jit_last_diagnostic(SlugJitHandle *handle, uint8_t *out,
                                     uint32_t capacity, uint32_t *written);
void slugarch_jit_destroy(SlugJitHandle *handle);

_Static_assert(sizeof(((SlugJitEvent *)0)->payload) == SLUG_JIT_PAYLOAD_BYTES,
               "event payload width changed");
_Static_assert(sizeof(((SlugJitPolicyInfo *)0)->digest) ==
                   SLUG_JIT_DIGEST_BYTES,
               "policy digest width changed");

#ifdef __cplusplus
}
#endif

#endif
