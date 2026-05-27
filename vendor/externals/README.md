# External Vendor Slots

SlugArch is intended to build from a single checkout. Runtime, mapping,
and RTL artifacts used by the workspace should be vendored under
`vendor/` and addressed with paths relative to the workspace root.

The current build uses `vendor/concordia-ptx/` and
`vendor/gemma-generated/`. Descriptor-only mappings for optional
upstream projects should point at a subdirectory here instead of an
absolute developer-machine path.
