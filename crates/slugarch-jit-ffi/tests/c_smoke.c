#include "slugarch_jit.h"

#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

_Static_assert(sizeof(SlugJitCreateArgs) == 24, "create ABI size changed");
_Static_assert(sizeof(SlugJitEvent) == 144, "event ABI size changed");
_Static_assert(offsetof(SlugJitEvent, payload) == 80,
               "event payload offset changed");
_Static_assert(sizeof(SlugJitPolicyInfo) == 64, "policy ABI size changed");
_Static_assert(sizeof(SlugJitDecision) == 48, "decision ABI size changed");
_Static_assert(sizeof(SlugJitStats) == 56, "stats ABI size changed");

static const char policy[] =
    "{"
    "\"version\":1,"
    "\"name\":\"c-smoke-validation\","
    "\"allowed_classes\":[\"cxl_mem_read\",\"cxl_mem_write\","
    "\"cxl_mem_data\",\"completion\"],"
    "\"ranges\":[{\"base\":83886080,\"length\":33554432}],"
    "\"sample_stride\":1,"
    "\"record_mode\":\"validation\","
    "\"metadata_budget\":256,"
    "\"epoch_policy\":\"phase\","
    "\"rules\":["
    "{\"op\":\"capture\",\"mode\":\"validation\"},"
    "{\"op\":\"emit\"},"
    "{\"op\":\"epoch_from_phase\"},"
    "{\"op\":\"halt\"}"
    "]"
    "}";

int main(void)
{
    SlugJitHandle *handle = NULL;
    SlugJitCreateArgs create = {
        .struct_size = sizeof(create),
        .abi_version = SLUG_JIT_ABI_VERSION,
        .backend = SLUG_JIT_BACKEND_RUST,
        .strict = 1,
        .diagnostic_capacity = 256,
        .reserved = 0,
    };
    SlugJitPolicyInfo info = {
        .struct_size = sizeof(info),
        .abi_version = SLUG_JIT_ABI_VERSION,
    };
    SlugJitEvent event = {
        .struct_size = sizeof(event),
        .abi_version = SLUG_JIT_ABI_VERSION,
        .event_id = 1,
        .client_id = 7,
        .direction = 0,
        .event_class = 2,
        .opcode = 0x44,
        .payload_len = 8,
        .address = UINT64_C(80) * 1024 * 1024,
        .tag = 11,
        .phase_id = 3,
        .monotonic_ns = 900,
        .status = 0,
        .reserved = 0,
        .payload = {1, 0, 2, 3, 4, 5, 6, 7},
    };
    SlugJitDecision decision = {
        .struct_size = sizeof(decision),
        .abi_version = SLUG_JIT_ABI_VERSION,
    };
    SlugJitStats stats = {
        .struct_size = sizeof(stats),
        .abi_version = SLUG_JIT_ABI_VERSION,
    };
    uint32_t first_diagnostic_len = UINT32_MAX;
    uint32_t second_diagnostic_len = UINT32_MAX;
    int32_t result;

    if (slugarch_jit_abi_version() != SLUG_JIT_ABI_VERSION)
        return 1;

    result = slugarch_jit_create(&create, &handle);
    if (result != SLUG_JIT_OK || handle == NULL)
        return 2;

    result = slugarch_jit_load_policy(
        handle, (const uint8_t *)policy, (uint32_t)(sizeof(policy) - 1), &info);
    if (result != SLUG_JIT_OK)
        return 3;
    if (info.backend != SLUG_JIT_BACKEND_RUST ||
        info.instruction_count != 4 || info.range_count != 1 ||
        info.metadata_budget != 256)
        return 4;

    result = slugarch_jit_observe(handle, &event, &decision);
    if (result != SLUG_JIT_OK)
        return 5;
    if (decision.accepted != 1 || decision.emitted != 1 ||
        decision.error_code != 0 || decision.payload_bytes != 8 ||
        decision.epoch != 3 || decision.record_id != 1)
        return 6;

    result = slugarch_jit_stats(handle, &stats);
    if (result != SLUG_JIT_OK)
        return 7;
    if (stats.event_count != 1 || stats.record_count != 1 ||
        stats.drop_count != 0 || stats.epoch != 3)
        return 8;

    result = slugarch_jit_last_diagnostic(
        handle, NULL, 0, &first_diagnostic_len);
    if (result != SLUG_JIT_OK)
        return 9;
    result = slugarch_jit_last_diagnostic(
        handle, NULL, 0, &second_diagnostic_len);
    if (result != SLUG_JIT_OK || first_diagnostic_len != second_diagnostic_len)
        return 10;

    event.struct_size = sizeof(event) - 1;
    memset(&decision, 0, sizeof(decision));
    decision.struct_size = sizeof(decision);
    decision.abi_version = SLUG_JIT_ABI_VERSION;
    result = slugarch_jit_observe(handle, &event, &decision);
    if (result != SLUG_JIT_ERR_STRUCT_SIZE)
        return 11;

    slugarch_jit_destroy(handle);
    puts("SLUG_JIT_C_ABI_PASS");
    return 0;
}
