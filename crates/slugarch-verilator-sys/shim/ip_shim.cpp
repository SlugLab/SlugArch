#include "ip_shim.h"

#include <cstring>
#include <cstdint>

#include "Vgemma_codegen_systolic_array_4x4_df.h"
#include "Vgemma_codegen_systolic_array_16x16_df.h"
#include "Vgemma_codegen_systolic_array_32x32_df.h"
#include "Vgemma_codegen_npu_array_v4_seed_g_df.h"
#include "Vgemma_codegen_npu_cluster_v4_df.h"
#include "Vgemma_codegen_noc_mesh_df.h"
#include "Vgemma_codegen_gemm_ip_df.h"
#include "Vslugcxl_4x4_top.h"
#include "verilated.h"

#include <array>
#include <queue>

// The Verilated classes don't share a base (they're independent generated
// classes), so dispatch is done via a tagged union rather than a C++ vtable.
// Each IP's public port layout is uniform (clk/rst_n/cmd_valid/cmd_ready/
// token_in/done_valid/token_out) so the per-variant template bodies are
// identical.

enum SlugIpKind : int {
    KIND_SYSTOLIC_4x4 = 0,
    KIND_SYSTOLIC_16x16,
    KIND_SYSTOLIC_32x32,
    KIND_NPU_SEED_G,
    KIND_NPU_CLUSTER,
    KIND_NOC_MESH,
    KIND_GEMM_IP,
    KIND_SLUGCXL_4x4,
};

struct SlugarchIp {
    SlugIpKind kind;
    void* dut;            // downcast based on kind
    VerilatedContext* ctx;
    uint64_t cycles;
    // CXL-only: FLIT queues. NULL for non-CXL IPs.
    std::queue<std::array<uint8_t, 64>>* inbound_flits;
    std::queue<std::array<uint8_t, 64>>* outbound_flits;
};

template <typename T>
static SlugarchIp* construct(SlugIpKind kind) {
    auto* ctx = new VerilatedContext();
    auto* d = new T(ctx);
    return new SlugarchIp{kind, d, ctx, 0, nullptr, nullptr};
}

// slugcxl_4x4 has a different port layout (FLIT in/out, no cmd_valid/token_in),
// so it needs dedicated construct/reset/tick helpers.
static SlugarchIp* construct_slugcxl_4x4() {
    auto* ctx = new VerilatedContext();
    auto* d = new Vslugcxl_4x4_top(ctx);
    auto* ip = new SlugarchIp{
        KIND_SLUGCXL_4x4, d, ctx, 0,
        new std::queue<std::array<uint8_t, 64>>(),
        new std::queue<std::array<uint8_t, 64>>(),
    };
    return ip;
}

static void reset_slugcxl_4x4(SlugarchIp* ip) {
    auto* d = static_cast<Vslugcxl_4x4_top*>(ip->dut);
    d->rst_n = 0;
    d->clk = 0;
    d->flit_in_valid = 0;
    d->flit_out_ready = 1;
    for (int i = 0; i < 4; ++i) {
        d->clk = 0; d->eval();
        d->clk = 1; d->eval();
        ip->cycles++;
    }
    d->rst_n = 1;
    d->clk = 0; d->eval();
    d->clk = 1; d->eval();
    ip->cycles++;
}

static void tick_slugcxl_4x4(SlugarchIp* ip) {
    auto* d = static_cast<Vslugcxl_4x4_top*>(ip->dut);

    // FLIT input handshake: if we have a queued FLIT, assert flit_in_valid
    // and drive the bytes. If flit_in_ready was asserted by the RTL last
    // cycle, pop the queue.
    if (!ip->inbound_flits->empty()) {
        d->flit_in_valid = 1;
        auto& f = ip->inbound_flits->front();
        std::memcpy(reinterpret_cast<void*>(&d->flit_in_data), f.data(), 64);
    } else {
        d->flit_in_valid = 0;
    }

    // Ensure flit_out_ready is asserted so the RTL can emit outputs.
    d->flit_out_ready = 1;

    d->clk = 0; d->eval();
    d->clk = 1; d->eval();
    ip->cycles++;

    // After the rising edge: if flit_in handshake succeeded, pop.
    if (d->flit_in_valid && d->flit_in_ready && !ip->inbound_flits->empty()) {
        ip->inbound_flits->pop();
    }

    // If flit_out handshake succeeded, capture the outbound FLIT.
    if (d->flit_out_valid && d->flit_out_ready) {
        std::array<uint8_t, 64> f;
        std::memcpy(f.data(), reinterpret_cast<const void*>(&d->flit_out_data), 64);
        ip->outbound_flits->push(f);
    }
}

template <typename T>
static void reset_impl(SlugarchIp* ip) {
    T* d = static_cast<T*>(ip->dut);
    d->rst_n = 0; d->clk = 0; d->cmd_valid = 0;
    std::memset(reinterpret_cast<void*>(&d->token_in), 0, SLUGARCH_TOKEN_BYTES);
    for (int i = 0; i < 4; ++i) {
        d->clk = 0; d->eval();
        d->clk = 1; d->eval();
        ip->cycles++;
    }
    d->rst_n = 1;
    d->clk = 0; d->eval();
    d->clk = 1; d->eval();
    ip->cycles++;
}

template <typename T>
static void tick_impl(SlugarchIp* ip) {
    T* d = static_cast<T*>(ip->dut);
    d->clk = 0; d->eval();
    d->clk = 1; d->eval();
    ip->cycles++;
}

template <typename T>
static void poke_impl(SlugarchIp* ip, int cmd_valid, const uint8_t token_in[SLUGARCH_TOKEN_BYTES]) {
    T* d = static_cast<T*>(ip->dut);
    d->cmd_valid = cmd_valid ? 1 : 0;
    std::memcpy(reinterpret_cast<void*>(&d->token_in), token_in, SLUGARCH_TOKEN_BYTES);
}

template <typename T>
static int peek_done_impl(SlugarchIp* ip, uint8_t token_out[SLUGARCH_TOKEN_BYTES]) {
    T* d = static_cast<T*>(ip->dut);
    std::memcpy(token_out, reinterpret_cast<const void*>(&d->token_out), SLUGARCH_TOKEN_BYTES);
    return d->done_valid ? 1 : 0;
}

template <typename T>
static int peek_cmd_ready_impl(const SlugarchIp* ip) {
    const T* d = static_cast<const T*>(ip->dut);
    return d->cmd_ready ? 1 : 0;
}

// ---- constructors ----

extern "C" SlugarchIp* slugarch_ip_new_systolic_4x4(void) {
    return construct<Vgemma_codegen_systolic_array_4x4_df>(KIND_SYSTOLIC_4x4);
}
extern "C" SlugarchIp* slugarch_ip_new_systolic_16x16(void) {
    return construct<Vgemma_codegen_systolic_array_16x16_df>(KIND_SYSTOLIC_16x16);
}
extern "C" SlugarchIp* slugarch_ip_new_systolic_32x32(void) {
    return construct<Vgemma_codegen_systolic_array_32x32_df>(KIND_SYSTOLIC_32x32);
}
extern "C" SlugarchIp* slugarch_ip_new_npu_seed_g(void) {
    return construct<Vgemma_codegen_npu_array_v4_seed_g_df>(KIND_NPU_SEED_G);
}
extern "C" SlugarchIp* slugarch_ip_new_npu_cluster(void) {
    return construct<Vgemma_codegen_npu_cluster_v4_df>(KIND_NPU_CLUSTER);
}
extern "C" SlugarchIp* slugarch_ip_new_noc_mesh(void) {
    return construct<Vgemma_codegen_noc_mesh_df>(KIND_NOC_MESH);
}
extern "C" SlugarchIp* slugarch_ip_new_gemm_ip(void) {
    return construct<Vgemma_codegen_gemm_ip_df>(KIND_GEMM_IP);
}
extern "C" SlugarchIp* slugarch_ip_new_slugcxl_4x4(void) {
    return construct_slugcxl_4x4();
}

// ---- lifecycle / methods ----

extern "C" void slugarch_ip_free(SlugarchIp* ip) {
    if (!ip) return;
    switch (ip->kind) {
        case KIND_SYSTOLIC_4x4:   delete static_cast<Vgemma_codegen_systolic_array_4x4_df*>(ip->dut); break;
        case KIND_SYSTOLIC_16x16: delete static_cast<Vgemma_codegen_systolic_array_16x16_df*>(ip->dut); break;
        case KIND_SYSTOLIC_32x32: delete static_cast<Vgemma_codegen_systolic_array_32x32_df*>(ip->dut); break;
        case KIND_NPU_SEED_G:     delete static_cast<Vgemma_codegen_npu_array_v4_seed_g_df*>(ip->dut); break;
        case KIND_NPU_CLUSTER:    delete static_cast<Vgemma_codegen_npu_cluster_v4_df*>(ip->dut); break;
        case KIND_NOC_MESH:       delete static_cast<Vgemma_codegen_noc_mesh_df*>(ip->dut); break;
        case KIND_GEMM_IP:        delete static_cast<Vgemma_codegen_gemm_ip_df*>(ip->dut); break;
        case KIND_SLUGCXL_4x4:
            delete static_cast<Vslugcxl_4x4_top*>(ip->dut);
            delete ip->inbound_flits;
            delete ip->outbound_flits;
            break;
    }
    delete ip->ctx;
    delete ip;
}

extern "C" void slugarch_ip_reset(SlugarchIp* ip) {
    switch (ip->kind) {
        case KIND_SYSTOLIC_4x4:   reset_impl<Vgemma_codegen_systolic_array_4x4_df>(ip); break;
        case KIND_SYSTOLIC_16x16: reset_impl<Vgemma_codegen_systolic_array_16x16_df>(ip); break;
        case KIND_SYSTOLIC_32x32: reset_impl<Vgemma_codegen_systolic_array_32x32_df>(ip); break;
        case KIND_NPU_SEED_G:     reset_impl<Vgemma_codegen_npu_array_v4_seed_g_df>(ip); break;
        case KIND_NPU_CLUSTER:    reset_impl<Vgemma_codegen_npu_cluster_v4_df>(ip); break;
        case KIND_NOC_MESH:       reset_impl<Vgemma_codegen_noc_mesh_df>(ip); break;
        case KIND_GEMM_IP:        reset_impl<Vgemma_codegen_gemm_ip_df>(ip); break;
        case KIND_SLUGCXL_4x4:    reset_slugcxl_4x4(ip); break;
    }
}

extern "C" uint64_t slugarch_ip_tick(SlugarchIp* ip) {
    switch (ip->kind) {
        case KIND_SYSTOLIC_4x4:   tick_impl<Vgemma_codegen_systolic_array_4x4_df>(ip); break;
        case KIND_SYSTOLIC_16x16: tick_impl<Vgemma_codegen_systolic_array_16x16_df>(ip); break;
        case KIND_SYSTOLIC_32x32: tick_impl<Vgemma_codegen_systolic_array_32x32_df>(ip); break;
        case KIND_NPU_SEED_G:     tick_impl<Vgemma_codegen_npu_array_v4_seed_g_df>(ip); break;
        case KIND_NPU_CLUSTER:    tick_impl<Vgemma_codegen_npu_cluster_v4_df>(ip); break;
        case KIND_NOC_MESH:       tick_impl<Vgemma_codegen_noc_mesh_df>(ip); break;
        case KIND_GEMM_IP:        tick_impl<Vgemma_codegen_gemm_ip_df>(ip); break;
        case KIND_SLUGCXL_4x4:    tick_slugcxl_4x4(ip); break;
    }
    return ip->cycles;
}

extern "C" void slugarch_ip_poke_cmd(SlugarchIp* ip, int cmd_valid, const uint8_t token_in[SLUGARCH_TOKEN_BYTES]) {
    switch (ip->kind) {
        case KIND_SYSTOLIC_4x4:   poke_impl<Vgemma_codegen_systolic_array_4x4_df>(ip, cmd_valid, token_in); break;
        case KIND_SYSTOLIC_16x16: poke_impl<Vgemma_codegen_systolic_array_16x16_df>(ip, cmd_valid, token_in); break;
        case KIND_SYSTOLIC_32x32: poke_impl<Vgemma_codegen_systolic_array_32x32_df>(ip, cmd_valid, token_in); break;
        case KIND_NPU_SEED_G:     poke_impl<Vgemma_codegen_npu_array_v4_seed_g_df>(ip, cmd_valid, token_in); break;
        case KIND_NPU_CLUSTER:    poke_impl<Vgemma_codegen_npu_cluster_v4_df>(ip, cmd_valid, token_in); break;
        case KIND_NOC_MESH:       poke_impl<Vgemma_codegen_noc_mesh_df>(ip, cmd_valid, token_in); break;
        case KIND_GEMM_IP:        poke_impl<Vgemma_codegen_gemm_ip_df>(ip, cmd_valid, token_in); break;
        case KIND_SLUGCXL_4x4:    /* slugcxl uses send_flit instead; no-op */ break;
    }
}

extern "C" int slugarch_ip_peek_done(SlugarchIp* ip, uint8_t token_out[SLUGARCH_TOKEN_BYTES]) {
    switch (ip->kind) {
        case KIND_SYSTOLIC_4x4:   return peek_done_impl<Vgemma_codegen_systolic_array_4x4_df>(ip, token_out);
        case KIND_SYSTOLIC_16x16: return peek_done_impl<Vgemma_codegen_systolic_array_16x16_df>(ip, token_out);
        case KIND_SYSTOLIC_32x32: return peek_done_impl<Vgemma_codegen_systolic_array_32x32_df>(ip, token_out);
        case KIND_NPU_SEED_G:     return peek_done_impl<Vgemma_codegen_npu_array_v4_seed_g_df>(ip, token_out);
        case KIND_NPU_CLUSTER:    return peek_done_impl<Vgemma_codegen_npu_cluster_v4_df>(ip, token_out);
        case KIND_NOC_MESH:       return peek_done_impl<Vgemma_codegen_noc_mesh_df>(ip, token_out);
        case KIND_GEMM_IP:        return peek_done_impl<Vgemma_codegen_gemm_ip_df>(ip, token_out);
        case KIND_SLUGCXL_4x4:    /* slugcxl uses try_recv_flit instead */ return 0;
    }
    return 0;
}

extern "C" int slugarch_ip_peek_cmd_ready(const SlugarchIp* ip) {
    switch (ip->kind) {
        case KIND_SYSTOLIC_4x4:   return peek_cmd_ready_impl<Vgemma_codegen_systolic_array_4x4_df>(ip);
        case KIND_SYSTOLIC_16x16: return peek_cmd_ready_impl<Vgemma_codegen_systolic_array_16x16_df>(ip);
        case KIND_SYSTOLIC_32x32: return peek_cmd_ready_impl<Vgemma_codegen_systolic_array_32x32_df>(ip);
        case KIND_NPU_SEED_G:     return peek_cmd_ready_impl<Vgemma_codegen_npu_array_v4_seed_g_df>(ip);
        case KIND_NPU_CLUSTER:    return peek_cmd_ready_impl<Vgemma_codegen_npu_cluster_v4_df>(ip);
        case KIND_NOC_MESH:       return peek_cmd_ready_impl<Vgemma_codegen_noc_mesh_df>(ip);
        case KIND_GEMM_IP:        return peek_cmd_ready_impl<Vgemma_codegen_gemm_ip_df>(ip);
        case KIND_SLUGCXL_4x4:    return 0;
    }
    return 0;
}

// ---- Plan 4: CXL FLIT FFI ----

extern "C" void slugarch_cxl_send_flit(SlugarchIp* ip, const uint8_t flit[SLUGARCH_FLIT_BYTES]) {
    if (ip->kind != KIND_SLUGCXL_4x4 || !ip->inbound_flits) return;
    std::array<uint8_t, 64> f;
    std::memcpy(f.data(), flit, 64);
    ip->inbound_flits->push(f);
}

extern "C" int slugarch_cxl_recv_flit(SlugarchIp* ip, uint8_t flit_out[SLUGARCH_FLIT_BYTES]) {
    if (ip->kind != KIND_SLUGCXL_4x4 || !ip->outbound_flits) return 0;
    if (ip->outbound_flits->empty()) return 0;
    auto f = ip->outbound_flits->front();
    ip->outbound_flits->pop();
    std::memcpy(flit_out, f.data(), 64);
    return 1;
}
