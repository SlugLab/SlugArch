# SlugArch Paper Benchmark Pass Design

## Goal

Add a truthful first benchmark pass to the Slug Architecture paper: include the validated QEMU Type-2 BAR result now, and make every broader, unmeasured paper claim visible as an explicit benchmark slot or limitation.

## Scope

This pass uses the live artifact from `qemu-type2-live-20260702-0028-summary` as the only measured result. It supports a narrow claim: the SlugArch host can export a 4x4 GEMM command stream as 49 64-byte request FLITs, send it through a guest-visible CXLMemSim QEMU Type-2 BAR2 path, receive one response per request, and validate the final boundary-visible result without tag mismatches or dispatch failures.

This pass does not claim measured CXL.cache coherence, CXL.mem traffic, DMA, address-translation services, page migration, switch ordering, overhead, compression efficiency, provenance precision, cross-device portability, recovery latency, or FPGA resource cost. Those remain benchmark slots in the paper.

## Inputs

- Paper directory: `/root/Concordia/64fa450c44d0cdf46c7c3a7d`
- SlugArch branch/worktree: `/root/Concordia/SlugArch/.worktrees/slugarch-paper-benchmarks`
- Validated summary: `/tmp/slugarch-cxlmemsim-type2/artifact/slugarch_cxlmemsim/qemu-type2-live-20260702-0028-summary/summary.json`
- Guest summary: `/tmp/slugarch-cxlmemsim-type2/artifact/slugarch_cxlmemsim/qemu-type2-live-20260702-0028-summary/guest-summary.json`
- Harness target: `targets/qemu-type2/`

## Design

Create a small durable evidence bundle under `docs/evaluation/` with copied summary data and a claim ledger. The ledger separates measured claims from architectural claims that still need benchmarks. Edit the paper's `eval.tex` so the evaluation section has two parts:

1. The existing claim-driven methodology remains, but it is reframed as the complete evaluation plan.
2. A new first-results subsection reports only the measured QEMU Type-2 BAR prototype result and explicitly says which claims are not yet supported by measurement.

The manuscript should avoid aggregate speedup language and avoid implying that QEMU Type-2 proves CXL.cache ordering, CXL.mem pooling, security provenance, compression overhead, recovery, or FPGA feasibility.

## Verification

Run the repository-side validation tests with an explicit Verilator include path because the installed `verilator -V` reports a stale compiled default root before the usable environment root:

```bash
env VERILATOR_INCLUDE=/home/victoryang00/tools/verilator/share/verilator/include \
  cargo test -p slugarch-host --test qemu_type2_artifacts
```

Compile the paper from `/root/Concordia/64fa450c44d0cdf46c7c3a7d` using the available LaTeX toolchain. If the toolchain is unavailable, report that boundary and still verify the changed `eval.tex` for unresolved placeholders and unsupported measured claims.
