# Upstream Submodules

This directory holds whole upstream repositories as Git submodules. The
build-facing vendored subsets remain under `vendor/concordia-ptx/` and
`vendor/gemma-generated/` so the workspace can keep compiling from a
small, stable artifact set.

Configured submodules:

- `vendor/upstream/concordia`:
  `https://github.com/CXLMemUring/Concordia.git`
- `vendor/upstream/gemma4-on-FPGA`:
  `https://github.com/vickiegpt/gemma4-on-FPGA.git`

`gemma4-on-FPGA` contains multi-GB Git LFS weight shards. Initialize it
with `GIT_LFS_SKIP_SMUDGE=1 git submodule update --init
vendor/upstream/gemma4-on-FPGA` unless those blobs are explicitly needed.

Pending access:

- `vendor/externals/ternary_matmul` should point at
  `https://github.com/sifferman/ternary_matmul.git`, branch `agilex`.
  The remote is not visible to the current Git/GitHub credentials, so it
  is documented but not pinned as a submodule yet.
