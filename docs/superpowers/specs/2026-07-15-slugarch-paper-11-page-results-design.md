# SlugArch 11-Page Results Presentation Design

## Objective

Revise the SlugArch manuscript in
`/root/Concordia/64fa450c44d0cdf46c7c3a7d` so that:

- SlugArch is the sole system name; the CFR name and macro are removed.
- The main text occupies no more than 11 IEEE/HPCA pages. References may
  continue afterward.
- Four dense result tables are replaced by compact, reviewer-readable graphs.
- Every QEMU Type-2 BAR2 statement matches what the live helper actually
  executes.
- The manuscript builds reproducibly from the command line with resolved
  references and legible vector figures.

## Evidence Boundary

The live QEMU experiment validates a guest-visible integration path, not a
device-side SlugArch executor. For each 64-byte request, the guest helper writes
the record into the CXLMemSim Type-2 BAR2 window, reads it back, and issues a
NOP command. The helper then interprets the request in guest software and emits
the response file consumed by the host validator. No response FLIT traverses
BAR2.

The paper may claim:

- five back-to-back runs in one TCG guest boot;
- 245 successful BAR2 request writes/readbacks and NOP completions;
- zero BAR2 readback, command, tag, dispatch, and validation failures for the
  fixed GEMM trace;
- deterministic request and response files across those runs;
- software record-format footprints and software validator times; and
- rejection of seven offline mutated response streams.

The paper must not infer device-side SlugArch execution, bidirectional BAR2
transport, CXL-link latency, production overhead, cross-boot reproducibility,
CXL.cache activity, CXL.mem/DAX traffic, DMA, ATS, migration, switch ordering,
hardware compression, hardware replay latency, recovery, or post-fit FPGA cost.

## Canonical Data

Use the July 4 high-resolution series without pooling it with the July 2
integer-millisecond series:

- Helper elapsed time, ms: `1.601086`, `1.651826`, `1.520746`, `1.471480`,
  `1.730958`.
- Mean: `1.595219` ms; median: `1.601086` ms; sample standard deviation:
  `0.103104` ms; range: `1.471480--1.730958` ms.
- Runs: `5/5` passed; requests and responses: `245/245`; BAR2 readback,
  command, tag, and dispatch errors: zero.
- Software-policy payload bytes for validation, delta, and full modes:
  `392`, `552`, and `1568` bytes.
- Total serialized software logs: `9732`, `10284`, and `11300` bytes for a
  `6272`-byte application trace.
- Equivalent-validator mean time over 200 in-process repetitions:
  `21.181`, `21.775`, and `25.157` microseconds.
- Mismatch-validator mean time over 200 in-process repetitions:
  `9.746`, `9.701`, and `9.297` microseconds.
- Offline mutations detected: truncated stream, bad tag, missing response,
  extra response, dispatch-failed opcode, wrong read data, and wrong response
  phase.

The later 200-repeat result stores arithmetic means but not the raw timing
samples. Validator-time graphs therefore have no error bars and explicitly say
that dispersion is unavailable. The exact reviewed values and source metadata
are copied into the paper repository as a plot-data snapshot so that figure
regeneration does not depend on an absolute local path.

## Figure Design

### Figure 1: SlugArch Prototype Results

Create one full-width, three-panel vector PDF, approximately 7.1 inches wide
and 1.8--2.0 inches tall.

1. **Guest-helper repeatability.** Use a categorical run-order dot plot, not a
   trend line. Plot the five elapsed times, add a quiet horizontal mean line,
   and directly label the mean and min--max range. Include compact callouts for
   `5/5 passed`, `245 BAR2 write/readbacks`, and `0 errors`. The panel subtitle
   states: 49 requests per run, TCG, one guest boot, mixed BAR2 and guest
   software path.
2. **Software log footprint.** Use three stacked bars. Each bar shows payload
   bytes and remaining serialized record bytes for validation, delta, and full
   modes. Directly label payload and total bytes. State that the application
   trace is 6272 bytes. Describe `4.00x` and `2.84x` only as payload reduction,
   never as total-log compression.
3. **Software validator time.** Use grouped bars for equivalent and mismatch
   validation in each mode. Start the quantitative axis at zero, label the
   values directly, and state `mean of 200 in-process repetitions; dispersion
   not retained`.

Use a white background, dark text, quiet gray guides, one blue palette root,
and a gold comparator. Use marker shape, outline, and hatch differences so the
panels remain legible in grayscale.

### Figure 2: Offline Fail-Stop Coverage

Create a single-column matrix figure, approximately 3.4 inches wide and
1.5--1.8 inches tall. Rows are the seven injected mutations. Columns are the
observed rejection mechanisms: framing error, tag mismatch, response-count
mismatch, dispatch failure, and decoded-result mismatch. Mark the observed
mechanism for each mutation and add a `7/7 detected` callout. The title and
caption say `offline response-stream mutation tests`; they do not imply live
device fault injection or recovery.

### Reproducible Plot Assets

Add the following files to the paper repository:

- `data/slugarch-results-20260704.json`: reviewed plot inputs, source artifact
  paths, experiment scope, and claim limitations.
- `scripts/plot_slugarch_results.py`: deterministic Matplotlib renderer.
- `img/slugarch-results.pdf`: full-width three-panel result figure.
- `img/slugarch-failstop.pdf`: single-column fail-stop matrix.

The renderer fixes dimensions, fonts, colors, labels, and PDF metadata. It
must be runnable offline with the installed Python and Matplotlib environment.

## Manuscript Restructuring

### Naming

- Delete the `\cfr` macro.
- Remove `(CFR)` from the abstract.
- Replace every CFR reference with `\sys` or `SlugArch`.
- Rename `CXL Fabric Replay` to `SlugArch Replay` or `Replay Engine`, depending
  on whether the phrase names the system or the mechanism.
- Verify that source, auxiliary files, extracted PDF text, bookmarks, and PDF
  metadata contain no stale CFR system name.

### Evaluation

Replace the four current result tables with Figures 1 and 2. Collapse the
single-run setup into a short paragraph and figure callouts. Organize the
evaluation around three measured statements:

1. guest-visible BAR2 write/readback and NOP integration plus guest-software
   interpretation;
2. software record-footprint and validator-time tradeoffs; and
3. offline fail-stop mutation coverage.

State the broader blocked claims once at the end of the section. Remove the
duplicated claim taxonomy, unexecuted prototype tiers, prospective workload
list, measurement tutorial, ablation wish list, and multi-paragraph benchmark
roadmap. Preserve enough setup to identify QEMU 10.0.94, Linux 6.18.0-rc5+,
TCG, the Type-2 device, one boot, run count, workload, and artifact scope.

Research-question status becomes:

- RQ1: scoped to BAR2 integration and software validation;
- RQ2: overhead blocked because no same-substrate baseline exists; report the
  1.595 ms value only as helper elapsed time;
- RQ3: scoped to software footprint and validator timing;
- RQ4: scoped to GEMM label-coverage instrumentation, not provenance
  precision; and
- RQ5: blocked beyond the QEMU Type-2 integration path.

### Formal Semantics

Retain the machine state, event vocabulary, recording/replay rules, boundary
equivalence definition, main correctness theorem, explicit assumptions, and a
proof sketch. Compress the 17 individually headed corner cases into a grouped
obligations paragraph or compact list. Remove the four standalone lemmas and
their full proofs from the main paper after incorporating the dependencies they
establish into the theorem proof sketch. This saves approximately 1.5--2 pages
without removing the formal contract.

### Introduction, Design, and Closing Sections

- Merge repeated motivation in the introduction and background while retaining
  the causality-gap problem and contribution list.
- Keep the four-step memory-coherency debugging recipe intact.
- Merge repeated coverage-policy, security, trust-boundary, and compatibility
  prose where the same limitation appears again in evaluation.
- Merge Deployment Path and Limitations into one compact section.
- Keep Related Work focused on direct contrasts.
- End with a short conclusion that names the demonstrated boundary and the
  blocked hardware claims.

## Page Budget

The repaired IEEE diagnostic baseline is 16 pages, with the conclusion on page
15 and references on page 16. The target budget is:

| Material | Main-text budget |
| --- | ---: |
| Abstract, introduction, and background | 2.0 pages |
| Design, including coherency-debugging recipe | 2.5 pages |
| Operational semantics and proof sketch | 2.5 pages |
| Evaluation and result figures | 2.5 pages |
| Related work, deployment/limitations, conclusion | 1.5 pages |
| **Total** | **11.0 pages** |

The figures replace existing table area rather than adding floats. Final
layout tuning may shorten captions and remove duplicated prose, but must not
change font size, margins, line spacing, or IEEE class geometry.

## Build Repair

Make the HPCA source compile outside Overleaf by:

- providing safe local defaults for the HPCA year and submission-number macros
  when the submission environment has not defined them;
- loading `amsthm` and defining the lemma, theorem, and corollary environments;
- converting ACM-style `\citep` calls to IEEE `\cite` calls;
- removing redundant package imports;
- using the IEEE bibliography style; and
- rebuilding from clean auxiliary state.

The premature forced bibliography break is removed. The conclusion must finish
on or before page 11; references may begin in the remaining page-11 space or on
a later page.

## Verification

The finished revision must pass all of these checks:

1. Run the plot renderer twice and confirm stable output hashes.
2. Inspect both PDFs directly for clipped text, tiny labels, color dependence,
   and unreadable grayscale distinctions.
3. Build the paper from clean auxiliary state with `latexmk -pdf
   -interaction=nonstopmode -halt-on-error main.tex`.
4. Confirm with `pdfinfo` that the file is letter-sized and that main text ends
   no later than page 11.
5. Use `pdftotext -layout` to verify figure captions, page boundaries, column
   flow, and the absence of stale CFR naming.
6. Confirm that the log has no undefined control sequences, citations,
   references, or labels and no overfull boxes caused by the revision.
7. Confirm that the manuscript states BAR2 write/readback plus guest-software
   response generation and never claims that response FLITs traverse BAR2.
8. Confirm that CXL.cache, CXL.mem/DAX, DMA, ATS, migration, switch ordering,
   hardware compression/replay, recovery, and FPGA cost remain explicitly
   unmeasured.
9. Review the final paper diff to ensure unrelated author content and result
   artifacts are preserved.

## Out of Scope

This revision does not rerun QEMU, create new performance measurements, claim
cross-boot repeatability, modify the SlugArch runtime, alter the CXLMemSim
helper, commit the currently untracked experiment directories, or invent
hardware results.
