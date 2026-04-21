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

## Plan 3 — End-to-end: **NOT STARTED**

Scope: `slugarch-fabric` (event loop, recorder, replayer, plus the
Rust impl of the CPU-backed `ptx_emulation_core`) + `slugarch-cli`
(two binaries: `slugarch` and `slugarch-bench`) + captured Qwen PTX
fixture (`tests/fixtures/qwen_decode_token.ptx`) + Tier 2 integration
tests (end-to-end run, determinism invariant, value-preservation
invariant, oracle invariant). Deliverable: full v1 success criteria
from the design doc.
