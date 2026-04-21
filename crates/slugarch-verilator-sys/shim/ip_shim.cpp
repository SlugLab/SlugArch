#include "ip_shim.h"

#include <cstring>
#include <cstdint>

#include "Vgemma_codegen_systolic_array_4x4_df.h"
#include "verilated.h"

struct SlugarchIp {
    Vgemma_codegen_systolic_array_4x4_df* dut;
    VerilatedContext* ctx;
    uint64_t cycles;
};

extern "C" SlugarchIp* slugarch_ip_new_systolic_4x4(void) {
    auto* ip = new SlugarchIp();
    ip->ctx = new VerilatedContext();
    ip->dut = new Vgemma_codegen_systolic_array_4x4_df(ip->ctx);
    ip->cycles = 0;
    return ip;
}

extern "C" void slugarch_ip_free(SlugarchIp* ip) {
    if (!ip) return;
    delete ip->dut;
    delete ip->ctx;
    delete ip;
}

extern "C" void slugarch_ip_reset(SlugarchIp* ip) {
    auto* d = ip->dut;
    d->rst_n = 0;
    d->clk = 0;
    d->cmd_valid = 0;
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

extern "C" uint64_t slugarch_ip_tick(SlugarchIp* ip) {
    auto* d = ip->dut;
    d->clk = 0; d->eval();
    d->clk = 1; d->eval();
    ip->cycles++;
    return ip->cycles;
}

extern "C" void slugarch_ip_poke_cmd(SlugarchIp* ip, int cmd_valid, const uint8_t token_in[SLUGARCH_TOKEN_BYTES]) {
    auto* d = ip->dut;
    d->cmd_valid = cmd_valid ? 1 : 0;
    std::memcpy(reinterpret_cast<void*>(&d->token_in), token_in, SLUGARCH_TOKEN_BYTES);
}

extern "C" int slugarch_ip_peek_done(SlugarchIp* ip, uint8_t token_out[SLUGARCH_TOKEN_BYTES]) {
    auto* d = ip->dut;
    std::memcpy(token_out, reinterpret_cast<const void*>(&d->token_out), SLUGARCH_TOKEN_BYTES);
    return d->done_valid ? 1 : 0;
}

extern "C" int slugarch_ip_peek_cmd_ready(const SlugarchIp* ip) {
    return ip->dut->cmd_ready ? 1 : 0;
}
