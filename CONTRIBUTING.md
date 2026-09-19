# Contributing to SlugArch

Useful contributions include application workloads, stronger failure models,
independent replay backends, calibrated timing parameters, and Zettai topology
coverage. Small, reproducible examples are particularly valuable.

## Start with checks that do not require QEMU

Use Python 3.11 or newer; these checks use only the standard library:

```bash
python3 -m unittest discover -s targets/splash-type2 -p 'test_*.py' -v
python3 -m unittest discover -s targets/splash-bsp -p 'test_*.py' -v
python3 -m unittest discover -s targets/splash-endpoint -p 'test_*.py' -v
```

The GitHub workflow runs these checks. It does not run the external QEMU build or
claim to reproduce a simulator campaign. Full integration instructions are in
[the endpoint guide](targets/splash-endpoint/README.md).

## Change an experiment

1. State the hypothesis, work allocation, baseline, and modeled resources.
2. Retain fixed inputs, all declared configurations, and explicit failure outcomes.
3. Use a fresh artifact directory. Preserve failures and do not overwrite a campaign.
4. Compare values and resource schedules to independent oracles. For bridge changes,
   include a disabled-forwarding control and verify unaffected branches.
5. Report virtual time, host wall time, and hardware measurements separately.

Large raw campaigns stay outside Git. Include source, compact summaries, provenance,
and reproduction commands in a pull request. Do not replace measured data with
estimated numbers without labeling the estimate.

## Report a bug or propose a workload

Open an [issue](https://github.com/SlugLab/SlugArch/issues) with the commit, QEMU
binary/source revision when applicable, exact command, expected behavior, and
smallest failing configuration. Include the first failed assertion and relevant
log excerpts. Remove credentials and unrelated private paths before sharing logs.
For a new workload, explain its data dependencies and what the replay boundary
must cover.

The Rust PTX/RTL stack is separate from the Python/QEMU experiments. Contributions
there should identify the crate and required vendored frontend/RTL inputs, and
run the relevant Cargo checks. See the source and existing design notes before
assuming that a Python-only checkout includes every Rust build dependency.
