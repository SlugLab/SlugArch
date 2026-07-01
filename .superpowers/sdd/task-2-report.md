# Task 2 Report: SlugArch CLI and `targets/qemu-type2`

## Changed files

- `crates/slugarch-cli/src/main.rs`
- `targets/qemu-type2/README.md`
- `targets/qemu-type2/identity_times_const.json`
- `targets/qemu-type2/run_existing_guest.sh`

## Implementation summary

- Added `slugarch export-cxlmemsim <job> --out <dir>` and
  `slugarch validate-cxlmemsim <job> --responses <responses.bin> --out <dir>`
  to the CLI.
- Added shared `read_gemm_job()` plus the CLI helpers that call
  `slugarch_host::qemu_type2::{export_requests, validate_responses}`.
- Added unit coverage in `slugarch-cli` for both new subcommands and the export
  artifact path.
- Added the stable simulator target surface under `targets/qemu-type2/`:
  README flow, identity fixture JSON, and `run_existing_guest.sh`.
- Marked `targets/qemu-type2/run_existing_guest.sh` executable.

## Tests and results

1. `VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include cargo test -p slugarch-cli`
   - Result: passed (`3 passed; 0 failed`)
2. `VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include cargo run -p slugarch-cli -- export-cxlmemsim targets/qemu-type2/identity_times_const.json --out /tmp/slugarch-qemu-type2-cli-smoke`
   - Result: passed
   - Output included:
     - `workload: slugcxl_gemm_4x4`
     - `requests: 49`
     - `flit_bytes: 64`
3. `test -s /tmp/slugarch-qemu-type2-cli-smoke/requests.bin`
   - Result: passed
4. `test -s /tmp/slugarch-qemu-type2-cli-smoke/expected.json`
   - Result: passed
5. `cargo fmt -p slugarch-cli`
   - Result: applied formatting required by rustfmt
6. `cargo fmt --check -p slugarch-cli`
   - Result: passed

## Self-review

- CLI command names, flags, and printed fields match the task brief.
- The target fixture JSON content matches the brief exactly.
- The target README keeps `targets/qemu-type2/` as the stable SlugArch
  simulator entrypoint and does not route into CXLMemSim file changes.
- The harness script matches the requested host-side flow and ignores the
  malformed legacy launch scripts as directed.
- The change stays within the requested ownership surface.

## Concerns

- The local Cargo build currently needs
  `VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include`
  for `slugarch-cli` verification because `slugarch-verilator-sys` otherwise
  picks an inconsistent Verilator include path from the environment/toolchain.
  Task 2 does not modify that broader build configuration.

## Task 2 Fix Report

### Changed files

- `targets/qemu-type2/run_existing_guest.sh`
- `targets/qemu-type2/README.md`

### Commit hash

- `9eec3b4` (`fix: split qemu guest ssh and scp settings`)

### Test commands and results

1. Temporary fake transport regression proof before the fix:
   `env PATH=/tmp/task2-fake-transport:$PATH FAKE_TRANSPORT_LOG=/tmp/task2-before.log CXLMEMSIM_GUEST_SSH='ssh root@test-guest' CXLMEMSIM_ROOT=/tmp/task2-fake-cxlmemsim bash targets/qemu-type2/run_existing_guest.sh /tmp/task2-run-before`
   - Result: failed as expected with `invalid scp target: ssh root@test-guest:/root/slugarch-qemu-type2/`
2. Temporary fake transport smoke after the fix:
   `env PATH=/tmp/task2-fake-transport:$PATH FAKE_TRANSPORT_LOG=/tmp/task2-after.log CXLMEMSIM_GUEST_SSH_CMD='ssh root@test-guest' CXLMEMSIM_GUEST_SCP_TARGET='root@test-guest' CXLMEMSIM_ROOT=/tmp/task2-fake-cxlmemsim bash targets/qemu-type2/run_existing_guest.sh /tmp/task2-run-after`
   - Result: passed; fake transport log showed `scp ... root@test-guest:/root/slugarch-qemu-type2/` and `commands.txt` recorded separate `guest_ssh_cmd` and `guest_scp_target` fields
3. `bash -n targets/qemu-type2/run_existing_guest.sh`
   - Result: passed
4. `VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include cargo run -p slugarch-cli -- export-cxlmemsim targets/qemu-type2/identity_times_const.json --out /tmp/slugarch-qemu-type2-cli-smoke`
   - Result: passed; output included `requests: 49` and `flit_bytes: 64`
5. `test -s /tmp/slugarch-qemu-type2-cli-smoke/requests.bin`
   - Result: passed
6. `test -s /tmp/slugarch-qemu-type2-cli-smoke/expected.json`
   - Result: passed
7. `cargo fmt --check -p slugarch-cli`
   - Result: not applicable; no Rust files were touched in this fix

### Concerns

- The fake transport regression proof is an untracked `/tmp` harness used only
  to demonstrate the SSH/SCP split and was not added to the repo.
- The README now makes the evidence boundary explicit, but the existing-guest
  script still does not capture QEMU or CXLMemSim logs; those remain artifacts
  to archive from the supported Type-2 launch and smoke path.
