# SlugArch QEMU JIT direct-CFMWS five-repeat evidence

This artifact contains ten fresh-process QEMU qtest executions: five with the
SlugArch Rust JIT backend and five with the FPGA-Verilator RTL backend. Each
process performs one 8-byte read and one 8-byte write at DPA 80 MiB through
the server-authoritative direct-CFMWS path with a requested model delay of
400 ns.

The checked result is:

- 10/10 fresh QEMU processes passed;
- 40/40 canonical request or completion events were accepted and recorded;
- 20/20 request/completion pairs join to the expected request ID and server
  sequence;
- 20/20 operations completed through `direct_cfmws`;
- the two backends have one identical non-timing semantic signature;
- the final counters total 40 records and 320 metadata bytes; and
- reject, drop, undershoot, and all alternative-path completion counts are
  zero.

All individual timing points are retained in `summary.json`. For Rust, the
applied-delay medians are 419 ns for reads and 415 ns for writes. For
FPGA-Verilator, they are 410 ns and 419 ns. The FPGA-Verilator write points
include one 760 ns host-scheduling outlier; it is retained rather than
discarded. These values verify that QEMU applied at least the requested
400 ns model delay. They are not physical CXL-link or FPGA timing. The host
event-pair spans likewise include QEMU, FFI, policy, logging, and scheduler
work and are reported only as whole-path simulator observations.

## Evidence layout

`raw/rust/` and `raw/fpga-verilator/` retain the two JSONL logs from every
fresh process. `summary.json` records each raw-file SHA-256 and validates the
replay joins, payload commitments, path counters, policy identity, phase, and
backend semantic equivalence.

The validator is:

```text
targets/qemu-type2/summarize_jit_cfmws.py
```

It can be rerun from the SlugArch repository root:

```sh
python3 targets/qemu-type2/summarize_jit_cfmws.py \
  --backend rust=artifact/slugarch_cxlmemsim/qemu-jit-cfmws-five-20260728/raw/rust \
  --backend fpga-verilator=artifact/slugarch_cxlmemsim/qemu-jit-cfmws-five-20260728/raw/fpga-verilator \
  --expected-repeats 5 \
  --output artifact/slugarch_cxlmemsim/qemu-jit-cfmws-five-20260728/summary.json
```

## Frozen identities

- SlugArch source: `c5efd01e64a9b000c5a2d3ccd576ba44dabf2693`
- QEMU runtime source: `52e729e85b0f40f2a23e4882483a155b3475562b`
- QEMU evidence-harness source (test-only follow-up):
  `1182f6c855c11161877db8b1aa95818ed3f082bb`
- QEMU binary SHA-256:
  `5e807325d64b7aa36f0727a1cd0835d1bc7f974147bd7b7fc1626c59db031a0b`
- QEMU `cxl-test` binary SHA-256:
  `c939b50dacf09045b7a1935ffc700d2b571c1c643ecb81c8bb8ea41c86f9d744`
- Rust library SHA-256:
  `a54ed58b2e1caa12b1d42214290788176aa97ff103905405e9958cde8952e7d7`
- FPGA-Verilator library SHA-256:
  `5d457fda0e65831bb5432d8f72d3ce8fbe85cd71a65e14e7e03d1898a217475f`
- validation-policy SHA-256:
  `65bf11a3c3f42e0e7247ba23890a1842084b33f84fda90e144015d3b8a1cf7ca`
- policy digest reported by both backends:
  `bc91e1b53305764adbaf714367cf2bf91206fbefade323b3b507307970ff81d4`
- QEMU 10.0.94, Rust 1.92.0, and Verilator 5.028

No Meson environment dump is included because it is irrelevant to replay and
may contain unrelated process credentials.
