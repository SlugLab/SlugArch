# SlugArch test fixtures

- `gemm.ptx` — Concordia's tiled-GEMM kernel from
  `examples/ptx_kernels/gemm.ptx`. PTX v7.5, sm_120. Vendored here so
  tests don't depend on a live Concordia checkout. Plan 3 uses this as
  the v1 end-to-end fixture.
