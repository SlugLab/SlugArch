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

#ifdef __cplusplus
}
#endif

#endif
