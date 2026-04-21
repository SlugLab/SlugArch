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

## Plan 2 — Hardware backends: **NOT STARTED**

Scope: `slugarch-backend` + `slugarch-verilator-sys` + `slugarch-verilator`
+ `slugarch-bench`. Vendors Gemma RTL (Verilog/SystemVerilog, ~200
files under `vendor/gemma-generated/rtl/designs/`). Adds a `build.rs`
that drives Verilator per-IP against the `.f` filelists already
present in the vendored runtime descriptors, plus `bindgen` FFI for
the resulting Verilator-compiled C++ models. 7 Verilator-compiled RTL
IPs (6 distinct compile units after deduping `gemm_ip` with
`systolic_array_16x16`). Deliverable: per-IP smoke tests pass;
`slugarch-bench` exercises all 7 RTL IPs with directed stim TOMLs.

## Plan 3 — End-to-end: **NOT STARTED**

Scope: `slugarch-fabric` (event loop, recorder, replayer, plus the
Rust impl of the CPU-backed `ptx_emulation_core`) + `slugarch-cli`
(two binaries: `slugarch` and `slugarch-bench`) + captured Qwen PTX
fixture (`tests/fixtures/qwen_decode_token.ptx`) + Tier 2 integration
tests (end-to-end run, determinism invariant, value-preservation
invariant, oracle invariant). Deliverable: full v1 success criteria
from the design doc.
