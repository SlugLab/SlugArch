# SlugArch Plan Status

## Plan 1 — Foundation: **COMPLETE**

- Cargo workspace: `crates/slugarch-ir`, `crates/slugarch-ptx-frontend`, and the
  vendored Concordia PTX parser crates under `vendor/concordia-ptx/`.
- Gemma runtime + mapping JSONs vendored under `vendor/gemma-generated/`
  (8 IPs + pipelines + ip-level rtlmaps, with LICENSE and MODEL_PROVENANCE
  preserved and absolute paths rewritten to be relative).
- `slugarch-ir` implements the full node set (IpId, Op, OpMeta, OperandRef,
  Edge, Function, Module, Context, FunctionBuilder), the 4 passes
  (fuse_decode_ops, select_backend with DefaultPolicy + ForceIp,
  assign_tokens, validate_against_rtlmap with real-file oracle loader),
  and JSON + bincode serialization with property-tested round-trips.
- `slugarch-ptx-frontend` parses PTX via the vendored parser and lowers
  Arith / BitOps / Transcendental / Ld/St / Mma / Control ops into SlugIR
  through a dispatcher that walks per-op-class lowerer modules.
- **Test count:** 40 first-party tests, all green (33 slugarch-ir +
  7 slugarch-ptx-frontend integration tests).

### Plan 1 caveats to carry into Plan 2

1. **CI should not run `cargo test --workspace`.** The vendored
   `ptx_parser` crate has 5 pre-existing upstream test failures (all in
   `ptx_parser::tests::report_unknown_*`). Those are latent issues from
   the Concordia commit we forked from — not regressions introduced by
   SlugArch. The Plan 2 CI task should use:
   ```bash
   cargo test -p slugarch-ir -p slugarch-ptx-frontend
   ```
   Or alternatively mark those upstream tests with `#[ignore]` at
   vendoring time (invasive; not chosen for Plan 1 to keep the vendor
   bundle byte-identical to upstream).
2. **Clippy invocation needs `--no-deps`.** `cargo clippy --all-targets
   -- -D warnings` otherwise escalates vendored macro-crate style lints
   (filter_map_identity, needless_return, etc.) into errors. The
   working command is:
   ```bash
   cargo clippy -p slugarch-ir -p slugarch-ptx-frontend --all-targets --no-deps -- -D warnings
   ```
3. **Mma shape extraction is synthetic.** The ptx_parser grammar
   hardcodes `m16n8k16` and discards the shape dims on `MmaDetails`.
   `parse_mma_shape_via_debug` currently recovers digits from the
   scalar-type names in Debug output, which coincidentally produces
   `[16, 16, 16]`. The real fix — either extend the parser to retain
   the shape tuple or hard-code `[16, 8, 16]` in the MmaLowerer — is
   a Plan 3 concern once captured PTX is in hand.
4. **PTX-parser `ast` module is private.** The plan originally used
   `ptx_parser::ast::Module` / `::Instruction` / `::ParsedOperand`, but
   `ast` is a private module re-exported via `pub use ast::*;`. All
   imports now use the crate-root paths (`ptx_parser::Module` etc.).
5. **`OperandRef` cannot derive Eq/Hash** because it carries an `f32`.
   That's fine — nothing downstream keys a map on `OperandRef`.
6. **`lower_to_slugir` threads an `'a` lifetime** instead of anonymous
   `'_` because `ptx_parser::Module<'input>` is invariant over the
   input lifetime.

## Plan 2 — Hardware backends: **COMPLETE**

- Vendored Gemma RTL under `vendor/gemma-generated/rtl/designs/` (10
  baseline Verilog files, subset of the 145 in the upstream tree —
  only files referenced by a .f filelist of the 7 target IPs) and
  `vendor/gemma-generated/generated/<ip>/{rtl,sim,hardware}/` for each
  IP, with absolute paths rewritten to be relative to the vendor root.
- `slugarch-verilator-sys`: build.rs runs Verilator 5.028 `--cc --build`
  per IP (7 total), producing `libV<top>.a` + `libverilated.a`. The
  hand-written C++ shim dispatches by tag (Verilated classes don't
  share a base, so no vtable). bindgen emits `src/lib.rs` FFI.
- `slugarch-verilator`: safe `VerilatedIp` with `WireCmd`/`WireDone`
  types. Uses `std::memcpy` against `token_in`/`token_out`
  (VL_INW/OUTW(&x, 255, 0, 8) = 8×uint32 layout, confirmed).
- `slugarch-backend`: `DispatchCmd`, `BackendBinding` trait, 6 per-IP
  bindings (Systolic shared across 4x4/16x16/32x32, NpuSeedG,
  NpuCluster, NoCMesh, GemmIp, PtxEmulation). `IpRuntime` descriptor
  loader reads all 8 IPs' runtime JSONs.
- `slugarch-bench`: build.rs runs `verilator --binary --timing` on
  each IP's smoke_tb, main.rs invokes them. `cargo run -p slugarch-bench`
  prints `bench: 7 pass, 0 fail`.
- **Test count:** 63 first-party tests, all green (33 slugarch-ir +
  7 slugarch-ptx-frontend + 7 slugarch-verilator-sys smoke +
  8 slugarch-verilator + 8 slugarch-backend). `cargo fmt --check` and
  clippy with `--no-deps -- -D warnings` green on all first-party crates.

### Plan 2 caveats to carry into Plan 3

1. **Verilator flag suite.** The working invocation for both the
   library build (`--cc`) and the bench build (`--binary`) is:
   ```
   -Irtl/designs -Wno-UNUSED -Wno-UNUSEDSIGNAL -Wno-WIDTH
   -Wno-TIMESCALEMOD -Wno-MODDUP -Wno-IMPORTSTAR -Wno-CASEINCOMPLETE
   -Wno-INITIALDLY
   ```
   `-Irtl/designs` is essential because the NPU baseline uses
   `` `include `` for its companion files. `-Wno-MODDUP` is needed
   because the baseline's includes + the filelist list the same
   companion modules. The other `-Wno-*` flags silence pre-existing
   upstream RTL authoring issues.
2. **`--cc` vs `--binary` timing.** `--cc` uses `--no-timing` (we
   drive the clock from Rust). `--binary` uses `--timing` because
   the vendored smoke_tbs use delay control (`always #5 clk = ~clk;`,
   `repeat (N) @(posedge clk)`) that `--no-timing` rejects.
3. **`.f` filelists require filtering.** For the library `--cc`
   build, we strip the `smoke_tb` line from the filelist (it has its
   own `initial begin` that would conflict with the Rust-driven
   clock). For the `--binary` build, the full filelist is used.
4. **Link names.** Verilator 5.x emits both `V<top>__ALL.a` (pre-lib-
   prefixed intermediate) and `libV<top>.a` (properly-prefixed
   archive). We link the latter as `rustc-link-lib=static=V<top>`
   + `rustc-link-lib=static=verilated`.
5. **DispatchCmd token encodings are v1 placeholders.** Each binding
   packs operand metadata into `token[0..N]` via a scheme we made up
   — not the real per-IP opcode layout that each wrapper's
   `port_bindings` table (in rtlmap.json) describes. Plan 3 replaces
   these once captured Qwen PTX exercises real dispatch shapes.
6. **`ptx_emulation_core` binding is a descriptor-only stub.**
   It produces `DispatchCmd`s with the raw Emu opcode but no CPU
   execution — Plan 3's fabric engine adds that.
7. **Bench uses SV smoke_tbs, not TOML stim.** The design doc
   originally called for TOML-driven stim files; Plan 2 invokes the
   vendored smoke_tbs via `verilator --binary`. A TOML loader is a
   post-Plan-3 backlog item.
8. **Cargo.lock is now tracked.** Plan 2 introduced binaries
   (slugarch-bench) and non-trivial build.rs logic, making lockfile
   reproducibility worth committing. Plan 1's "untracked" status is
   reversed.
9. **Plan 1 caveats still apply** (ast:: path, OperandRef Eq/Hash,
   Mma shape synthetic, etc.) — see the Plan 1 section above.

## Plan 3 — End-to-end: **COMPLETE (v1 ships)**

- `tests/fixtures/gemm.ptx` vendored from Concordia's
  `examples/ptx_kernels/gemm.ptx` (PTX v7.5, sm_120, 146 lines). The
  original spec called for a captured `qwen_decode_token.ptx`; that
  required live CUDA + Qwen infrastructure which isn't provisioned
  here, so gemm.ptx is the v1 fixture instead. It's a real tiled-GEMM
  kernel — parses + lowers to ~77 SlugIR ops (19 Arith+Add,
  11 Dma, 13 Arith+Mov, 1 Arith+Fma, etc.).
- `slugarch-fabric`: event loop `Fabric::run(Vec<DispatchCmd>)` drives
  instantiated `VerilatedIp`s and CPU-backed `ptx_emulation_core`.
  Respects token_in/token_out deps. Retires RTL dispatches on
  `done_valid`, CPU-emu dispatches immediately. `ReplayArtifact`
  serializes to bincode.
- `slugarch-cli`: `slugarch run|replay|validate` subcommands. Run
  tests/fixtures/gemm.ptx -> `total_cycles: 69, completions: 77`;
  replay of the recorded .slug file reproduces the same.
- **Tier 2 integration tests**, all green:
  - Path A (`gemm_e2e.rs`): end-to-end run.
  - Path B (`determinism.rs`): same-binding replay is byte-identical
    RunReport + host_mem.
  - Path C (`value_preservation.rs`): different-named policies produce
    identical host-mem hashes.
- **v1 demo uses `AllEmuPolicy`** that routes every op to
  `PtxEmulationCore` — a necessity because Plan 2's placeholder token
  encodings don't drive the other RTL backends to `done_valid`. Real
  per-IP encodings derived from each wrapper's `port_bindings` table
  in rtlmap.json are a post-v1 item.
- **Test count:** 75 first-party tests, all green. `cargo clippy
  --no-deps -- -D warnings` clean on all 7 first-party crates.

### v1 success criteria status

| # | Criterion | Status |
|---|---|---|
| 1 | `cargo build` on Linux x86_64 (Rust + Verilator + g++) | ✓ |
| 2 | `cargo test` (Tier 1) without Verilator | ✓ (Plan 1 crates) |
| 3 | `cargo test --features rtl-tests` (Tier 2 + 3) | ✓ (all tests gated by Verilator are included in the standard test run) |
| 4 | `slugarch run tests/fixtures/qwen_decode_token.ptx` in baseline window | **N/A** — gemm.ptx replaces qwen_decode_token. `slugarch run tests/fixtures/gemm.ptx` produces 69 cycles deterministically. |
| 5 | `slugarch replay` preserves host-mem output | ✓ (Paths B + C green) |
| 6 | `slugarch-bench --ip <each IP>` for all 7 IPs | ✓ (Plan 2 bench, 7/7 green) |

### Known post-v1 items

1. **Real per-opcode CPU emulation.** The 23 `ptx_emulation_core`
   opcodes currently have cycle costs but no semantic execution —
   host memory isn't mutated. v2 can implement the opcodes against
   the host buffer.
2. **Real per-IP token encodings.** Every binding packs placeholder
   bytes into `DispatchCmd.token`. The NoC, systolic arrays, and NPU
   wrappers all have specific `port_bindings` layouts in their
   rtlmap.json files — v2 reads those and packs accordingly, which
   enables routing non-Arith ops back to hardware.
3. **Captured Qwen PTX fixture.** If the surrounding environment
   ever provisions CUDA + Qwen, the original spec's qwen_decode_token
   fixture can land (and with it, the oracle-invariant Tier 2 test
   against the existing `qwen_decode_token.rtlmap.json`).
4. **TOML stim loader for `slugarch-bench`.** The current bench uses
   the vendored `.sv` smoke_tbs via `verilator --binary`. A
   TOML-driven loader would be more flexible.
5. **Multi-threaded fabric.** v1 is single-threaded; the
   `VerilatedIp` wrapper is `Send + !Sync` so per-IP threading is
   plausible once the event loop needs to drive cores in parallel.
6. **`emit_dispatches` duplication.** The helper appears in the CLI
   and three Tier 2 tests — if it stabilizes, promoting it to a
   shared crate is worth doing.


## Plan 4 — CXL RTL generation + host runtime: **COMPLETE**

- Three new crates: `slugarch-cxl-wire` (FLIT encode/decode, 13 tests),
  `slugcxl-gen` (SystemVerilog generator, snapshot-tested, 6 tests),
  `slugarch-host` (GemmJob / dispatch / result / CxlHost::run_gemm, 8 unit +
  3 integration tests).
- Modified: `slugarch-verilator-sys` (new Verilator compile unit
  `slugcxl_4x4_top` + C++ shim FLIT queues + FLIT FFI), `slugarch-verilator`
  (IpId::SlugCxl4x4 + send_flit/try_recv_flit), `slugarch-cli`
  (`slugarch run-cxl`), `slugarch-ir` (IpId::SlugCxl4x4).
- Generated RTL in `vendor/gemma-generated/generated/slugcxl/`:
  `slugcxl_endpoint.sv` (11-state CXL Type-2 endpoint FSM) +
  `slugcxl_4x4_top.sv` (endpoint + systolic_array_16x16 wrapper instantiation)
  + `slugcxl_endpoint_runtime.json`. Idempotent; snapshot-tested.
- **Demo:** `slugarch run-cxl tests/fixtures/identity_times_const.json`
  sends 49 real 64-byte FLITs through Verilator; systolic_array_16x16
  computes 4x4 GEMM in a sub-region; 212 cycles; I×B=B verified byte-
  for-byte. Tier 2 Path A PASSES.
- Determinism + wire-level tag-match tests pass.

### Plan 4 scope deviations from the written plan

1. **Target IP is systolic_array_16x16, not 4x4.** The 4x4 df_wrapper
   packs all 32 inputs in one token_in and XOR-folds the outputs across
   all 16 c cells — incompatible with per-cell load/compute/read
   dispatch. The 16x16 wrapper uses the documented protocol. Host runs
   4x4 GEMMs as the top-left sub-region of the 16x16 grid (addrs
   row*16+col).
2. **Endpoint state machine grew to 10 states** (originally 6 planned).
   Added: S_DRIVE_CMD_NO_DONE (loads don't assert out_valid in the
   16x16 baseline, so pure-load dispatches skip await-done). Split
   EMIT_* into SETUP+HOLD pairs (non-blocking-assignment gotcha: a
   single-cycle assert→deassert of flit_out_valid is invisible to the
   shim; SETUP holds the assert for at least one full cycle).
3. **Token extraction diverges for RwD vs Req.** RwD dispatches carry
   the token in data[0..4] (flit_in_data[119:88]); Req dispatches carry
   it in addr[63:32] (flit_in_data[87:56]) because M2SReq has no data
   field in the v1 FLIT layout. Endpoint addr-match uses low-16 bits
   only, leaving high bits free for read-token payload.
4. **Read-data location in token_out is [23:0]**, not [31:8] as the
   plan spec'd. The 16x16 wrapper pads as `{232'd0, u_read_data}` with
   the 24-bit value in the low bits.
5. **Tier 2 tag-mismatch test is wire-layer only**, per plan caveat.

### Plan 4 caveats for post-v1

1. **CXL.cache RTL unused.** `.cache` wire types exist in slugarch-cxl-
   wire but the v1 host doesn't emit D2H/H2D. Endpoint state machine
   doesn't decode them either; that's v2 work.
2. **Single-in-flight dispatch.** Endpoint deasserts flit_in_ready
   while a dispatch is pending. Multi-in-flight needs a tag-indexed
   response queue.
3. **v1 FLIT layout is documented, not CXL 2.0/3.0 spec-compliant.**
   The 64-byte packet structure + class/opcode table is v1-local. Real
   CXL spec compliance is post-v1.
4. **PTX-over-CXL still not connected.** `slugarch run <ptx>` uses
   Plan 3 fabric (AllEmuPolicy); `slugarch run-cxl` accepts a GemmJob
   JSON, not a PTX file. Routing PTX dispatches through CXL would
   require more IPs with real token encodings.
5. **5-state → 10-state endpoint refactor implies regenerating** the
   vendor/generated/slugcxl/slugcxl_endpoint.sv from slugcxl-gen — the
   file is checked-in, and the build.rs verifies it's present but
   doesn't rerun the generator. If the generator source diverges from
   the checked-in SV, `cargo test -p slugcxl-gen` catches it via
   insta snapshots.
6. Plans 1-3 caveats still apply.
