#ifndef SLUGARCH_IP_SHIM_H
#define SLUGARCH_IP_SHIM_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Token width is 256 bits = 32 bytes.
#define SLUGARCH_TOKEN_BYTES 32

// Opaque handle to a Verilated IP model.
typedef struct SlugarchIp SlugarchIp;

// Per-IP constructors (7 RTL-backed IPs).
SlugarchIp* slugarch_ip_new_systolic_4x4(void);
SlugarchIp* slugarch_ip_new_systolic_16x16(void);
SlugarchIp* slugarch_ip_new_systolic_32x32(void);
SlugarchIp* slugarch_ip_new_npu_seed_g(void);
SlugarchIp* slugarch_ip_new_npu_cluster(void);
SlugarchIp* slugarch_ip_new_noc_mesh(void);
SlugarchIp* slugarch_ip_new_gemm_ip(void);

// Lifecycle.
void slugarch_ip_free(SlugarchIp* ip);
void slugarch_ip_reset(SlugarchIp* ip);

// Drive one clock cycle. Returns the post-tick cycle count.
uint64_t slugarch_ip_tick(SlugarchIp* ip);

// Set cmd_valid and token_in for the next rising edge. token_in is a 32-byte
// buffer in little-endian byte order.
void slugarch_ip_poke_cmd(SlugarchIp* ip, int cmd_valid, const uint8_t token_in[SLUGARCH_TOKEN_BYTES]);

// Peek the current done_valid / token_out. Returns done_valid (0 or 1).
int slugarch_ip_peek_done(SlugarchIp* ip, uint8_t token_out[SLUGARCH_TOKEN_BYTES]);

// Returns the current value of cmd_ready. All Gemma wrappers tie cmd_ready = 1.
int slugarch_ip_peek_cmd_ready(const SlugarchIp* ip);

// --- Plan 4: CXL FLIT FFI ---

#define SLUGARCH_FLIT_BYTES 64

SlugarchIp* slugarch_ip_new_slugcxl_4x4(void);

// Enqueue one FLIT for the RTL to consume on the next successful
// flit_in handshake. Safe to call multiple times; FLITs are queued.
void slugarch_cxl_send_flit(SlugarchIp* ip, const uint8_t flit[SLUGARCH_FLIT_BYTES]);

// Try to pop one FLIT from the RTL's output queue. Returns 1 if a FLIT
// was written to flit_out, 0 if the queue is empty.
int  slugarch_cxl_recv_flit(SlugarchIp* ip, uint8_t flit_out[SLUGARCH_FLIT_BYTES]);

// --- Runtime-programmable Hardware-JIT model ---
//
// EVENT_BYTES is SlugArch's local Verilator/debug packet encoding. It is not a
// standards-compliant CXL FLIT.

#define SLUGARCH_HJ_POLICY_BYTES 640
#define SLUGARCH_HJ_EVENT_BYTES 64
#define SLUGARCH_HJ_RECORD_BYTES 128

enum {
    SLUGARCH_HJ_OK = 0,
    SLUGARCH_HJ_ERR_NULL = -1,
    SLUGARCH_HJ_ERR_SIZE = -2,
    SLUGARCH_HJ_ERR_TIMEOUT = -3,
    SLUGARCH_HJ_ERR_RTL = -4,
    SLUGARCH_HJ_ERR_BUFFER = -5,
    SLUGARCH_HJ_ERR_PROTOCOL = -6,
};

typedef struct SlugarchHj SlugarchHj;

typedef struct SlugarchHjStats {
    uint64_t cycles;
    uint64_t event_count;
    uint64_t record_count;
    uint64_t metadata_bytes;
    uint64_t reject_count;
    uint64_t drop_count;
    uint64_t instruction_count;
    uint64_t epoch;
    uint64_t app_flit_bytes;
    uint64_t stall_cycles;
    uint32_t policy_error;
    uint16_t last_reject_code;
    uint8_t policy_ready;
    uint8_t reserved;
} SlugarchHjStats;

SlugarchHj* slugarch_hj_new(void);
void slugarch_hj_free(SlugarchHj* hj);
void slugarch_hj_reset(SlugarchHj* hj);
int slugarch_hj_load_policy(SlugarchHj* hj, const uint8_t* image,
                            uint32_t image_len);
int slugarch_hj_observe(SlugarchHj* hj,
                        const uint8_t event[SLUGARCH_HJ_EVENT_BYTES],
                        uint8_t record[SLUGARCH_HJ_RECORD_BYTES],
                        uint32_t* record_len, uint64_t* cycles);
int slugarch_hj_stats(const SlugarchHj* hj, SlugarchHjStats* stats);

#ifdef __cplusplus
}
#endif

#endif
