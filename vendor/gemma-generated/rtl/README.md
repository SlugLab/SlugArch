# Vendored Gemma RTL (Plan 2 scope)

Copied from upstream `ext/gemma/rtl/designs/` at the commit recorded in
`../UPSTREAM_COMMIT`. Only files referenced by one of the 7 target IPs'
`.f` filelists are vendored; other Gemma RTL variants (npu_array_v1..v3,
ternary_matmul_core, taalas_ip, etc.) are intentionally excluded from
this vendor bundle because we don't compile them in Plan 2.

See `../generated/<ip>/hardware/<ip>.f` for each IP's exact source
list. Paths inside the `.f` files are relative to
`vendor/gemma-generated/` (rewritten from the upstream absolute paths
during vendoring).
