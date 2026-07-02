# QEMU Type-2 Fail-Stop Validation Pass

## Measured Claim

The SlugArch QEMU Type-2 validator rejects malformed response streams for the
4x4 GEMM command sequence used in the live BAR2 benchmark. This is an offline
artifact-validation result, not live fault injection inside QEMU or CXLMemSim.

## Evidence

- Command: `env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include cargo test -p slugarch-host --test qemu_type2_artifacts`
- Result: 9 tests run, 9 passed, 0 failed, 0 ignored.
- Good stream baseline: 49 request FLITs, 49 response FLITs, `status=pass`, `tag_mismatches=0`, `dispatch_failures=0`.
- Fail-stop cases: truncated byte stream, bad tag, missing response, extra duplicate response, dispatch-failed opcode, wrong read data, and wrong response phase.
- Production-code change: none required for these cases. The existing validator already enforced the needed fail-stop behavior; this pass adds durable test coverage and an explicit paper evidence bundle.

## Case Table

| Case | Injected response-stream fault | Observed signal |
| --- | --- | --- |
| `truncated_bytes` | 63-byte response stream, shorter than one 64-byte FLIT | Validator returns an error containing `multiple of 64`. |
| `bad_tag` | Response 10 tag changed to 99 | `status=fail`, `tag_mismatches=1`, `dispatch_failures=0`. |
| `missing_response` | Final response removed | `status=fail`, `response_count=48`. |
| `extra_duplicate_response` | Duplicate final response appended | `status=fail`, `response_count=50`. |
| `dispatch_failed_response` | Response 3 opcode changed to `DispatchFailed` | `status=fail`, `dispatch_failures=1`. |
| `wrong_read_data` | First byte of first `MemData` response incremented | `status=fail`; decoded result differs from expected matrix. |
| `wrong_response_phase` | First `MemData` response replaced by `Cmp` at same tag | `status=fail`, `dispatch_failures=1`. |

## Boundary

This evidence supports the paper claim that the checked artifact validator has
fail-stop checks for malformed QEMU Type-2 response traces. It does not support
claims about live injected QEMU faults, device-side recovery, CXL.cache,
CXL.mem, DMA, ATS, page migration, switch ordering, compression, runtime
overhead, or FPGA behavior.
