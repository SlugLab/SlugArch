# SlugArch

PTX-in / cycles-out simulator with Fabric Replay for heterogeneous
backend replay. Built on Verilator-compiled RTL extracted from the
Gemma FPGA artifact; PTX parsing via the vendored Concordia parser.

**Status:** under construction. Plan 1 (foundation) scaffolds the
workspace and implements the PTX frontend + SlugIR. See
`docs/superpowers/specs/2026-04-20-slugarch-ptx-cxl-design.md` for the
design document and `docs/superpowers/plans/` for implementation plans.

## Build

```bash
cargo build --workspace
cargo test --workspace
```

Plan 1 requires only: Rust stable + Cargo. Verilator is introduced in
Plan 2.

## Layout

See the design doc for the full repository layout. In Plan 1 the
populated crates are `slugarch-ir` and `slugarch-ptx-frontend`. Future
plans add `slugarch-backend`, `slugarch-verilator(-sys)`,
`slugarch-fabric`, and `slugarch-cli`.
