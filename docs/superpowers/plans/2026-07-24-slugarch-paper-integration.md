# SlugArch Paper Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce a reproducible IEEE/HPCA SlugArch manuscript whose main text ends by page 11, whose two full-width figures contain only validated legacy BAR2 and complete Type-2 CXL.mem campaign data, and whose claims preserve the measured-versus-blocked boundary.

**Architecture:** Make every paper change in an isolated clone at `/tmp/slugarch-paper-integration`, leaving the source checkout and the dirty SlugArch control workspace untouched. Build Figure 1 from a frozen copy of the three reviewed July artifacts; accept Figure 2 only through the complete-campaign handoff defined below. Enforce naming, evidence, figure, citation, and page contracts with executable Python checks before synchronizing a reviewed allowlist back to the paper checkout.

**Tech Stack:** IEEEtran LaTeX, BibTeX, Python 3.11+, Matplotlib 3.10+, `latexmk`, `pdfinfo`, `pdftotext`, `pdftocairo`, Git, SHA-256.

---

## Approved Inputs and Non-Negotiable Boundaries

The implementation is governed by:

- `/root/Concordia/SlugArch/docs/superpowers/specs/2026-07-15-slugarch-paper-11-page-results-design.md`
- `/root/Concordia/SlugArch/docs/superpowers/specs/2026-07-24-slugarch-type2-cxlmem-experiment-design.md`

The July 24 design supersedes the July 15 figure layout and page allocation:

- Figure 1 is one 7.1-by-3.0-inch, 2-by-2 legacy-results figure.
- Figure 2 is one 7.1-by-3.0-inch, 2-by-2 Type-2 CXL.mem figure.
- Evaluation receives 3.0 pages; design and semantics receive 2.25 pages each.
- The conclusion must finish on or before page 11; references are outside that limit.

The paper must retain these distinct evidence statements:

- Legacy BAR2 carried request write/readback plus NOP. Guest software generated
  responses, and no response FLIT traversed BAR2.
- New Type-2 CXL.mem results are eligible only when the experiment campaign is
  complete, checksum-valid, lowest-ordinal eligible, and has all 20 committed
  boots plus all Figure 2 panel gates.
- The simulator latency settings are not physical CXL link latency.
- CXL.cache, BI/snoop behavior, DMA, ATS, migration, switches, device execution,
  recovery, and FPGA cost remain unmeasured.

The complete-campaign handoff is fixed as:

```text
/tmp/slugarch-type2-cxlmem-paper-export/
  slugarch-type2-cxlmem.json
  export-checksums.sha256
  export-validation.json
```

The Type-2 experiment implementation must create that directory only after its
campaign validator has selected the eligible campaign. This paper plan never
manufactures, edits, estimates, interpolates, or partially reconstructs that
export.

## File Map

Paths in the implementation tasks are relative to
`/tmp/slugarch-paper-integration` unless an absolute path is shown.

### Existing files to modify

- `main.tex`: IEEE build defaults, package cleanup, theorem environments,
  SlugArch naming, bibliography flow, and the main-text end label.
- `intro.tex`: compressed motivation and SlugArch-only contribution list.
- `background.tex`: compact CPU replay, residual-flow, and CXL background.
- `design.tex`: compact policy/design text while retaining the four-step
  coherency-debugging recipe.
- `semantics.tex`: grouped obligations, one theorem, and one proof sketch.
- `eval.tex`: six-part evidence-first evaluation with Figures 1 and 2.
- `related.tex`: direct related-work contrasts, compact deployment/limitations,
  and conclusion.
- `cite.bib`: retain the bibliography database and add the supplied reference
  as an anonymous unpublished manuscript; do not copy its placeholder venue,
  DOI, volume, or article metadata.

### Files to create

- `data/slugarch-results-20260704.json`: frozen Figure 1 input and provenance.
- `data/slugarch-type2-cxlmem.json`: byte-for-byte copy of the validated
  complete-campaign paper export.
- `scripts/check_paper_contract.py`: source, data, build, and page gate.
- `scripts/plot_slugarch_results.py`: deterministic Figure 1 renderer.
- `scripts/plot_slugarch_type2_cxlmem.py`: Figure 2 renderer and data validator.
- `scripts/test_plot_slugarch_results.py`: Figure 1 data/render tests.
- `scripts/test_plot_slugarch_type2_cxlmem.py`: mockup, complete-data, and
  deterministic-render tests.
- `scripts/build_slugarch_layout_mockup.py`: isolated empty-axis layout build.
- `img/slugarch-results.pdf`: final Figure 1.
- `img/slugarch-type2-cxlmem.pdf`: final Figure 2.

### Generated files reviewed at the final gate

- `main.pdf`
- `main.aux`
- `main.bbl`
- `main.log`

The source repository tracks `main.aux` and `main.bbl`. Regenerate both from the
final IEEE build and include them in the reviewed synchronization allowlist so
that tracked auxiliary files do not retain stale CFR or ACM state.

---

## Task 1: Isolate the Paper and Record the Baseline

**Files:**
- Inspect: `/root/Concordia/64fa450c44d0cdf46c7c3a7d`
- Create: `/tmp/slugarch-paper-integration`

- [ ] **Step 1: Invoke the isolation skill**

Read and follow `superpowers:using-git-worktrees`. Because the source
repository is outside the writable workspace and its metadata is read-only,
use a no-hardlink clone rather than changing the source checkout.

- [ ] **Step 2: Verify the exact source state**

Run:

```bash
git -C /root/Concordia/64fa450c44d0cdf46c7c3a7d rev-parse HEAD
git -C /root/Concordia/64fa450c44d0cdf46c7c3a7d status --short --branch
git -C /root/Concordia/SlugArch status --short --branch
```

Expected:

```text
99698736f7582c7112f52679544e2478525d9deb
```

The paper checkout may still show its pre-existing generated-file dirt, and
the SlugArch workspace may still show the untracked July 4 artifacts. Do not
stage, clean, or alter either checkout. If the paper HEAD differs, stop and
reconcile the approved design against the new paper revision before continuing.

- [ ] **Step 3: Clone and create the implementation branch**

Run:

```bash
git clone --no-hardlinks \
  /root/Concordia/64fa450c44d0cdf46c7c3a7d \
  /tmp/slugarch-paper-integration
git -C /tmp/slugarch-paper-integration \
  switch -c codex/slugarch-type2-paper-integration
git -C /tmp/slugarch-paper-integration status --short --branch
```

Expected:

```text
## codex/slugarch-type2-paper-integration
```

- [ ] **Step 4: Capture the current build failure without changing sources**

Run:

```bash
cd /tmp/slugarch-paper-integration
latexmk -C main.tex
latexmk -pdf -interaction=nonstopmode -halt-on-error main.tex
```

Expected: FAIL at the undefined `\hpcayear` use in the HPCA submission header.
Save the terminal output in the execution notes; do not treat the stale
checked-in PDF as a build baseline.

---

## Task 2: Add the Paper Contract Before Editing the Manuscript

**Files:**
- Create: `scripts/check_paper_contract.py`

- [ ] **Step 1: Create the contract checker**

Create `scripts/check_paper_contract.py` with this complete interface:

```python
#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TEX_NAMES = (
    "main.tex",
    "intro.tex",
    "background.tex",
    "design.tex",
    "semantics.tex",
    "eval.tex",
    "related.tex",
)
EXPECTED_FIGURE_SIZE = (511.2, 216.0)  # 7.1 by 3.0 inches, in PDF points.


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def run(*args: str) -> str:
    return subprocess.run(
        args,
        cwd=ROOT,
        check=True,
        text=True,
        capture_output=True,
    ).stdout


def all_tex() -> str:
    return "\n".join(
        (ROOT / name).read_text(encoding="utf-8") for name in TEX_NAMES
    )


def check_sources(stage: str) -> None:
    text = all_tex()
    main = (ROOT / "main.tex").read_text(encoding="utf-8")
    evaluation = (ROOT / "eval.tex").read_text(encoding="utf-8")
    bibliography = (ROOT / "cite.bib").read_text(encoding="utf-8")

    require(
        re.search(r"\\documentclass\[10pt,conference\]\{IEEEtran\}", main)
        is not None,
        "paper is not using the approved IEEEtran class",
    )
    require(
        re.search(r"\\cfr\b|\bCFR\b|CXL Fabric Replay", text, re.IGNORECASE)
        is None,
        "stale CFR system name",
    )
    require("\\citep{" not in text, "ACM-style citep remains")
    require(
        "\\bibliographystyle{IEEEtran}" in main,
        "IEEE bibliography style is missing",
    )
    require(
        "ACM-Reference-Format" not in text,
        "ACM bibliography style remains",
    )
    require(
        "\\newcommand{\\cfr}" not in main,
        "obsolete cfr macro remains",
    )
    require(
        "\\label{sec:maintext-end}" in main,
        "main-text end label is missing",
    )
    require(
        "\\clearpage\n\\bibliographystyle" not in main,
        "forced bibliography page break remains",
    )
    require(
        "\\cite{cxl-sync-costs-reference-2026}" in evaluation,
        "reference-manuscript methodology citation is missing",
    )
    require(
        "@unpublished{cxl-sync-costs-reference-2026" in bibliography
        and "Quantifying Synchronization Costs Across CXL Memory Access Modes"
        in bibliography,
        "reference-manuscript bibliography entry is missing",
    )
    require(
        "10.1145/nnnnnnn.nnnnnnn" not in bibliography,
        "placeholder reference-manuscript DOI was copied",
    )

    if stage == "final":
        require(
            evaluation.count("\\begin{figure*}") == 2,
            "evaluation must contain exactly two full-width figures",
        )
        require(
            "\\begin{table}" not in evaluation
            and "\\begin{table*}" not in evaluation,
            "evaluation result tables remain",
        )
        require(
            "img/slugarch-results.pdf" in evaluation,
            "Figure 1 is not included",
        )
        require(
            "img/slugarch-type2-cxlmem.pdf" in evaluation,
            "Figure 2 is not included",
        )
        for phrase in (
            "No response FLIT traverses BAR2",
            "guest software generated",
            "server-authoritative",
            "n=5 independent guest boots",
            "median with minimum and maximum",
            "one untimed warmup pass",
            "post-transport guest-software",
            "CXL.cache remains unmeasured",
        ):
            require(
                phrase in evaluation,
                f"missing required evidence phrase: {phrase}",
            )
        for phrase in (
            "physical CXL bandwidth",
            "physical CXL latency",
            "device-side SlugArch execution",
            "response FLITs traversed BAR2",
        ):
            require(
                phrase not in evaluation,
                f"forbidden overclaim remains: {phrase}",
            )


def check_legacy_data() -> None:
    path = ROOT / "data" / "slugarch-results-20260704.json"
    require(path.exists(), "legacy Figure 1 snapshot is missing")
    data = json.loads(path.read_text(encoding="utf-8"))
    require(data["schema_version"] == 1, "legacy schema version drifted")
    require(
        data["bar2_helper"]["elapsed_ms"]
        == [1.601086, 1.651826, 1.520746, 1.47148, 1.730958],
        "legacy helper timing series drifted",
    )
    require(
        data["software_policy"]["payload_bytes"] == [392, 552, 1568],
        "legacy payload series drifted",
    )
    require(
        data["software_policy"]["serialized_bytes"]
        == [9732, 10284, 11300],
        "legacy serialized-byte series drifted",
    )
    require(
        data["software_policy"]["equivalent_us"]
        == [21.181, 21.775, 25.157],
        "legacy equivalent-validator series drifted",
    )
    require(
        data["software_policy"]["mismatch_us"]
        == [9.746, 9.701, 9.297],
        "legacy mismatch-validator series drifted",
    )
    require(
        len(data["offline_failstop"]) == 7,
        "legacy fail-stop set must contain seven mutations",
    )


def parse_pdf_size(path: Path) -> tuple[float, float]:
    info = subprocess.run(
        ("pdfinfo", str(path)),
        check=True,
        text=True,
        capture_output=True,
    ).stdout
    match = re.search(
        r"^Page size:\s+([0-9.]+) x ([0-9.]+) pts",
        info,
        re.MULTILINE,
    )
    require(match is not None, f"cannot parse PDF size for {path.name}")
    return float(match.group(1)), float(match.group(2))


def check_figure(path: Path) -> None:
    require(path.exists(), f"missing figure: {path.name}")
    require(path.stat().st_size > 8_000, f"figure is unexpectedly small: {path.name}")
    width, height = parse_pdf_size(path)
    expected_width, expected_height = EXPECTED_FIGURE_SIZE
    require(abs(width - expected_width) <= 0.6, f"wrong width: {path.name}")
    require(abs(height - expected_height) <= 0.6, f"wrong height: {path.name}")


def check_type2_data() -> None:
    path = ROOT / "data" / "slugarch-type2-cxlmem.json"
    require(path.exists(), "validated Type-2 paper snapshot is missing")
    sys.path.insert(0, str(ROOT / "scripts"))
    from plot_slugarch_type2_cxlmem import load_and_validate  # noqa: PLC0415

    load_and_validate(path)


def aux_page(label: str) -> int:
    aux = (ROOT / "main.aux").read_text(encoding="utf-8", errors="replace")
    match = re.search(
        rf"\\newlabel\{{{re.escape(label)}\}}\{{\{{[^}}]*\}}\{{(\d+)\}}",
        aux,
    )
    require(match is not None, f"missing page label: {label}")
    return int(match.group(1))


def check_captions() -> None:
    evaluation = (ROOT / "eval.tex").read_text(encoding="utf-8")
    captions = re.findall(r"\\caption\{(.*?)\}\s*\\label", evaluation, re.DOTALL)
    require(len(captions) == 2, "expected exactly two evaluation captions")
    for index, caption in enumerate(captions, start=1):
        plain = re.sub(r"\\[A-Za-z]+\{([^}]*)\}", r"\1", caption)
        words = re.findall(r"[A-Za-z0-9][A-Za-z0-9+./-]*", plain)
        require(
            len(words) <= 90,
            f"Figure {index} caption exceeds 90 words: {len(words)}",
        )


def check_build(final: bool) -> None:
    for name in ("main.pdf", "main.log", "main.aux", "main.bbl"):
        require((ROOT / name).exists(), f"missing build output: {name}")
    log = (ROOT / "main.log").read_text(encoding="utf-8", errors="replace")
    for marker in (
        "Undefined control sequence",
        "LaTeX Error:",
        "There were undefined references",
        "There were undefined citations",
        "Citation `",
        "Reference `",
        "Overfull \\hbox",
    ):
        require(marker not in log, f"build log contains: {marker}")
    info = run("pdfinfo", "main.pdf")
    require(
        "Page size:       612 x 792 pts (letter)" in info,
        "main PDF is not US letter",
    )
    if final:
        end_page = aux_page("sec:maintext-end")
        require(end_page <= 11, f"main text ends on page {end_page}")
        pages_with_conclusion = []
        for page in range(1, 12):
            text = run(
                "pdftotext",
                "-f",
                str(page),
                "-l",
                str(page),
                "-layout",
                "main.pdf",
                "-",
            )
            if "CONCLUSION" in text.upper():
                pages_with_conclusion.append(page)
        require(pages_with_conclusion, "conclusion is not visible by page 11")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Check the SlugArch paper contract")
    parser.add_argument(
        "--stage",
        choices=("source", "legacy", "final"),
        default="final",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    check_sources(args.stage)
    check_build(final=args.stage == "final")
    if args.stage in ("legacy", "final"):
        check_legacy_data()
        check_figure(ROOT / "img" / "slugarch-results.pdf")
    if args.stage == "final":
        check_type2_data()
        check_figure(ROOT / "img" / "slugarch-type2-cxlmem.pdf")
        check_captions()
    print(f"paper contract ({args.stage}): PASS")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (AssertionError, KeyError, ValueError, json.JSONDecodeError) as exc:
        print(f"paper contract: FAIL: {exc}", file=sys.stderr)
        raise SystemExit(1)
```

- [ ] **Step 2: Run the checker and verify the first failure**

Run:

```bash
python3 scripts/check_paper_contract.py --stage source
```

Expected:

```text
paper contract: FAIL: stale CFR system name
```

- [ ] **Step 3: Commit only the contract**

Run:

```bash
git add scripts/check_paper_contract.py
git diff --cached --check
git commit -m "test: define SlugArch paper integration contract"
```

Expected: one new file in the commit.

---

## Task 3: Repair the IEEE Build and Complete the SlugArch Rename

**Files:**
- Modify: `main.tex:1-152`
- Modify: `intro.tex:11,37`
- Modify: `background.tex:5,11,13,17`
- Modify: `design.tex:5,92,147-149`
- Modify: `semantics.tex:5-6,442`
- Modify: `eval.tex:20,145-148`
- Modify: `related.tex:4,7,10,13,16,19,118`
- Modify: `cite.bib`

- [ ] **Step 1: Replace the package/default block in `main.tex`**

Keep `\documentclass[10pt,conference]{IEEEtran}` and replace the duplicated
package imports with exactly:

```tex
\usepackage{cite}
\usepackage{amsmath,amssymb,amsfonts,amsthm}
\usepackage{algorithmic}
\usepackage{graphicx,textcomp,xcolor,fancyhdr,booktabs,xspace,pifont}
\usepackage[hyphens]{url}
\usepackage{hyperref}
\usepackage{siunitx,courier,comment,listings,enumitem}

\providecommand{\hpcayear}{2027}
\providecommand{\hpcasubmissionnumber}{000}

\newtheorem{lemma}{Lemma}
\newtheorem{theorem}{Theorem}
\newtheorem{corollary}{Corollary}
```

Delete the repeated package block currently below `\newcommand{\hpcaheight}`.
Keep the HPCA author/header conditionals unchanged.

- [ ] **Step 2: Remove the CFR macro and repair abstract naming**

Retain:

```tex
\newcommand{\sys}{SlugArch\xspace}
\newcommand{\proto}{Slug\xspace}
\newcommand{\syss}{SynthesisGuide\xspace}
\newcommand{\hj}{Hardware JIT\xspace}
\newcommand{\emc}{External Memory Controller\xspace}
\newcommand{\cha}{cache home agent\xspace}
```

Delete:

```tex
\newcommand{\cfr}{CFR\xspace}
```

Replace the two abstract sentences containing CFR with:

```tex
device-specific debugging paths. This paper proposes \sys, a boundary-level
replay substrate for CXL-era systems. SlugArch does not replace the processor,
memory model, or instruction/data organization.
```

- [ ] **Step 3: Apply the exact remaining naming substitutions**

Use these replacements:

```text
intro.tex: "the CFR replay engine" -> "the SlugArch replay engine"
intro.tex: "\cfr" -> "\sys"
design.tex: "\subsection{CXL Fabric Replay}" -> "\subsection{SlugArch Replay}"
design.tex: "\cfr replays" -> "\sys replays"
semantics.tex: "\cfr replay" -> "\sys replay"
eval.tex: "\cfr" -> "\sys"
related.tex: "\cfr" -> "\sys"
```

Then verify:

```bash
rg -n -i '\\cfr\b|\bCFR\b|CXL Fabric Replay' -- *.tex
```

Expected: no output.

- [ ] **Step 4: Convert citation and bibliography syntax**

Replace all 20 `\citep{...}` calls in `background.tex`, `design.tex`,
`eval.tex`, and `related.tex` with `\cite{...}`.

Replace the bibliography tail in `main.tex` with:

```tex
\phantomsection
\label{sec:maintext-end}
\bibliographystyle{IEEEtran}
\bibliography{cite}
```

There must be no `\clearpage` before the bibliography.

- [ ] **Step 5: Add and narrowly cite the supplied reference manuscript**

First verify the immutable input:

```bash
sha256sum /root/Concordia/sigmetrics27summer-paper399.pdf
```

Expected:

```text
d6a90335b27188e2623f4cda2ee4da2639715cb15f0dc2f6a3da80c96ecd5e8f  /root/Concordia/sigmetrics27summer-paper399.pdf
```

Append this entry to `cite.bib`:

```bibtex
@unpublished{cxl-sync-costs-reference-2026,
  author = {{Anonymous Authors}},
  title = {Quantifying Synchronization Costs Across {CXL} Memory Access Modes},
  note = {Anonymous reference manuscript supplied to the authors},
  month = jul,
  year = {2026}
}
```

In the evaluation methodology, add exactly:

```tex
An anonymous reference manuscript motivated only the configured
80-ns, 400-ns, 2-\(\mu\)s, and 10-\(\mu\)s grid and our five-repeat
median/minimum/maximum convention~\cite{cxl-sync-costs-reference-2026};
we do not import its platform claims or performance values.
```

Do not describe the manuscript as accepted or published and do not copy its
placeholder DOI or any quantitative result.

- [ ] **Step 6: Build from clean auxiliary state**

Run:

```bash
latexmk -C main.tex
latexmk -pdf -interaction=nonstopmode -halt-on-error main.tex
python3 scripts/check_paper_contract.py --stage source
rg -n '\\citep|ACM-Reference-Format' -- *.tex
rg -n \
  'Undefined control sequence|LaTeX Error|undefined references|undefined citations|Citation .* undefined|Reference .* undefined' \
  main.log
```

Expected:

```text
paper contract (source): PASS
```

Both searches must be empty. Record the resulting total-page count and
main-text end page as diagnostic values only; the 11-page gate is enforced
after compression and figure integration.

- [ ] **Step 7: Commit the build, rename, and reference repair**

Run:

```bash
git add main.tex intro.tex background.tex design.tex semantics.tex eval.tex related.tex cite.bib
git diff --cached --check
git commit -m "fix: repair IEEE build and rename CFR to SlugArch"
```

Expected: only the seven TeX sources and `cite.bib` are committed.

---

## Task 4: Freeze the Legacy Evidence and Build Four-Panel Figure 1

**Files:**
- Create: `data/slugarch-results-20260704.json`
- Create: `scripts/test_plot_slugarch_results.py`
- Create: `scripts/plot_slugarch_results.py`
- Create: `img/slugarch-results.pdf`
- Read only:
  `/root/Concordia/SlugArch/docs/evaluation/qemu-type2-knob-sweep-20260704.json`
- Read only:
  `/root/Concordia/SlugArch/docs/evaluation/rq1-5-cxlmemsim-20260704.json`
- Read only:
  `/root/Concordia/SlugArch/docs/evaluation/qemu-type2-failstop-20260702.json`

- [ ] **Step 1: Verify the reviewed source hashes**

Run:

```bash
sha256sum \
  /root/Concordia/SlugArch/docs/evaluation/qemu-type2-knob-sweep-20260704.json \
  /root/Concordia/SlugArch/docs/evaluation/rq1-5-cxlmemsim-20260704.json \
  /root/Concordia/SlugArch/docs/evaluation/qemu-type2-failstop-20260702.json
```

Expected:

```text
a1e83743ab5f67f34dcaf027008572e0bbd2056d876ca0ec4132a186b1642822  /root/Concordia/SlugArch/docs/evaluation/qemu-type2-knob-sweep-20260704.json
8b98fb614cec6c547f92f55523843d26694ffc065599cba24e4f42225681f092  /root/Concordia/SlugArch/docs/evaluation/rq1-5-cxlmemsim-20260704.json
a11743a4dcee38c67e6e987fec0c9bd372ced37920fa6a25db8a789269414a63  /root/Concordia/SlugArch/docs/evaluation/qemu-type2-failstop-20260702.json
```

Stop if any hash differs. A changed source requires a new review; do not update
the expected values opportunistically.

- [ ] **Step 2: Create the frozen Figure 1 snapshot**

Create `data/slugarch-results-20260704.json`:

```json
{
  "schema_version": 1,
  "source": {
    "bar2_summary": {
      "path": "SlugArch/docs/evaluation/qemu-type2-knob-sweep-20260704.json",
      "sha256": "a1e83743ab5f67f34dcaf027008572e0bbd2056d876ca0ec4132a186b1642822"
    },
    "policy_summary": {
      "path": "SlugArch/docs/evaluation/rq1-5-cxlmemsim-20260704.json",
      "sha256": "8b98fb614cec6c547f92f55523843d26694ffc065599cba24e4f42225681f092"
    },
    "failstop_summary": {
      "path": "SlugArch/docs/evaluation/qemu-type2-failstop-20260702.json",
      "sha256": "a11743a4dcee38c67e6e987fec0c9bd372ced37920fa6a25db8a789269414a63"
    },
    "qemu": "10.0.94",
    "kernel": "Linux 6.18.0-rc5+",
    "device": "8086:0d92",
    "accelerator": "TCG",
    "guest_boots": 1
  },
  "bar2_helper": {
    "runs": 5,
    "passes": 5,
    "requests_per_run": 49,
    "elapsed_ms": [
      1.601086,
      1.651826,
      1.520746,
      1.47148,
      1.730958
    ],
    "mean_ms": 1.595219,
    "minimum_ms": 1.47148,
    "maximum_ms": 1.730958,
    "total_request_write_readbacks": 245,
    "bar2_readback_errors": 0,
    "command_errors": 0,
    "tag_errors": 0,
    "dispatch_errors": 0,
    "scope": "Request write/readback and NOP traversed BAR2; guest software generated responses; no response FLIT traversed BAR2."
  },
  "software_policy": {
    "modes": [
      "Validation",
      "Delta",
      "Full"
    ],
    "application_bytes": 6272,
    "payload_bytes": [
      392,
      552,
      1568
    ],
    "serialized_bytes": [
      9732,
      10284,
      11300
    ],
    "equivalent_us": [
      21.181,
      21.775,
      25.157
    ],
    "mismatch_us": [
      9.746,
      9.701,
      9.297
    ],
    "validator_repetitions": 200,
    "dispersion_retained": false
  },
  "offline_failstop": [
    {
      "mutation": "Truncated bytes",
      "mechanism": "Framing"
    },
    {
      "mutation": "Bad tag",
      "mechanism": "Tag"
    },
    {
      "mutation": "Missing response",
      "mechanism": "Count"
    },
    {
      "mutation": "Extra response",
      "mechanism": "Count"
    },
    {
      "mutation": "Dispatch failed",
      "mechanism": "Dispatch"
    },
    {
      "mutation": "Wrong read data",
      "mechanism": "Result"
    },
    {
      "mutation": "Wrong phase",
      "mechanism": "Dispatch"
    }
  ],
  "claim_limitations": [
    "The five BAR2 runs occurred back-to-back in one guest boot.",
    "BAR2 carried request write/readback and NOP, not response FLITs.",
    "Validator values are arithmetic means; raw dispersion was not retained.",
    "Mutations were applied offline and do not demonstrate device fault injection or recovery.",
    "This snapshot does not measure CXL.mem, CXL.cache, physical link latency, DMA, ATS, migration, switch ordering, hardware replay, or FPGA cost."
  ]
}
```

- [ ] **Step 3: Write the failing Figure 1 tests**

Create `scripts/test_plot_slugarch_results.py`:

```python
#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from plot_slugarch_results import render  # noqa: E402


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class LegacyFigureTests(unittest.TestCase):
    def setUp(self) -> None:
        self.data_path = ROOT / "data" / "slugarch-results-20260704.json"
        self.data = json.loads(self.data_path.read_text(encoding="utf-8"))

    def test_reviewed_values(self) -> None:
        self.assertEqual(
            self.data["bar2_helper"]["elapsed_ms"],
            [1.601086, 1.651826, 1.520746, 1.47148, 1.730958],
        )
        self.assertEqual(
            self.data["software_policy"]["serialized_bytes"],
            [9732, 10284, 11300],
        )
        self.assertEqual(len(self.data["offline_failstop"]), 7)
        self.assertFalse(self.data["software_policy"]["dispersion_retained"])

    def test_render_is_deterministic_and_correct_size(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            first = Path(directory) / "first.pdf"
            second = Path(directory) / "second.pdf"
            render(self.data_path, first)
            render(self.data_path, second)
            self.assertEqual(digest(first), digest(second))
            info = subprocess.run(
                ("pdfinfo", str(first)),
                check=True,
                text=True,
                capture_output=True,
            ).stdout
            self.assertIn("Pages:           1", info)
            self.assertRegex(info, r"Page size:\s+511\.2 x 216 pts")


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 4: Run the test and verify it fails before the renderer exists**

Run:

```bash
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 -m unittest scripts/test_plot_slugarch_results.py -v
```

Expected: FAIL with `ModuleNotFoundError: No module named
'plot_slugarch_results'`.

- [ ] **Step 5: Implement the Figure 1 renderer**

Create `scripts/plot_slugarch_results.py`:

```python
#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path

import matplotlib

matplotlib.use("pdf")
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib.colors import ListedColormap  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DATA = ROOT / "data" / "slugarch-results-20260704.json"
DEFAULT_OUTPUT = ROOT / "img" / "slugarch-results.pdf"

DARK = "#17212b"
BLUE = "#2878b5"
LIGHT_BLUE = "#9ecae1"
GOLD = "#d99b28"
GRAY = "#d9dee3"
WHITE = "#ffffff"
PDF_METADATA = {
    "Title": "SlugArch existing prototype evidence",
    "Author": "Anonymous",
    "Subject": "Reviewed SlugArch prototype results",
    "Keywords": "SlugArch, CXL, QEMU",
    "Creator": "SlugArch deterministic figure renderer",
    "Producer": "Matplotlib",
    "CreationDate": datetime(2026, 7, 24, tzinfo=timezone.utc),
    "ModDate": datetime(2026, 7, 24, tzinfo=timezone.utc),
}


def configure() -> None:
    matplotlib.rcParams.update(
        {
            "font.family": "DejaVu Sans",
            "font.size": 7.0,
            "axes.titlesize": 7.2,
            "axes.labelsize": 7.0,
            "xtick.labelsize": 7.0,
            "ytick.labelsize": 7.0,
            "legend.fontsize": 7.0,
            "pdf.fonttype": 42,
            "ps.fonttype": 42,
            "axes.edgecolor": DARK,
            "axes.labelcolor": DARK,
            "text.color": DARK,
            "xtick.color": DARK,
            "ytick.color": DARK,
        }
    )


def style_axis(axis: plt.Axes) -> None:
    axis.grid(axis="y", color=GRAY, linewidth=0.55, zorder=0)
    axis.spines["top"].set_visible(False)
    axis.spines["right"].set_visible(False)
    axis.spines["left"].set_linewidth(0.6)
    axis.spines["bottom"].set_linewidth(0.6)


def render(data_path: Path, output: Path) -> None:
    configure()
    data = json.loads(data_path.read_text(encoding="utf-8"))
    helper = data["bar2_helper"]
    policy = data["software_policy"]

    fig, axes = plt.subplots(2, 2, figsize=(7.1, 3.0))
    fig.subplots_adjust(
        left=0.075,
        right=0.988,
        bottom=0.155,
        top=0.91,
        wspace=0.32,
        hspace=0.62,
    )

    axis = axes[0, 0]
    runs = list(range(1, helper["runs"] + 1))
    elapsed = helper["elapsed_ms"]
    axis.scatter(
        runs,
        elapsed,
        s=24,
        color=BLUE,
        edgecolor=DARK,
        linewidth=0.55,
        marker="o",
        zorder=3,
    )
    axis.axhline(
        helper["mean_ms"],
        color=GOLD,
        linewidth=1.0,
        linestyle=(0, (4, 2)),
        zorder=2,
    )
    axis.set_xticks(runs)
    axis.set_xlabel("Run order in one TCG guest boot")
    axis.set_ylabel("Helper elapsed (ms)")
    axis.set_ylim(1.40, 1.79)
    axis.set_title("(a) Legacy BAR2 helper repeatability", loc="left")
    axis.text(
        0.02,
        0.96,
        "5/5 passed · 245 write/readbacks · 0 errors",
        transform=axis.transAxes,
        ha="left",
        va="top",
        fontsize=7.0,
    )
    axis.text(
        0.98,
        0.07,
        "mean 1.595 ms\nrange 1.471–1.731 ms",
        transform=axis.transAxes,
        ha="right",
        va="bottom",
        fontsize=7.0,
    )
    style_axis(axis)

    axis = axes[0, 1]
    positions = list(range(len(policy["modes"])))
    payload = policy["payload_bytes"]
    totals = policy["serialized_bytes"]
    other = [total - captured for total, captured in zip(totals, payload)]
    axis.bar(
        positions,
        payload,
        width=0.62,
        color=GOLD,
        edgecolor=DARK,
        linewidth=0.55,
        hatch="///",
        label="Captured payload",
        zorder=3,
    )
    axis.bar(
        positions,
        other,
        width=0.62,
        bottom=payload,
        color=LIGHT_BLUE,
        edgecolor=DARK,
        linewidth=0.55,
        label="Other record bytes",
        zorder=3,
    )
    axis.axhline(
        policy["application_bytes"],
        color=DARK,
        linewidth=0.8,
        linestyle=(0, (3, 2)),
        zorder=2,
    )
    for x_value, captured, total in zip(positions, payload, totals):
        axis.text(
            x_value,
            total + 250,
            f"{total:,}",
            ha="center",
            va="bottom",
            fontweight="bold",
        )
        axis.text(
            x_value,
            max(captured + 250, 900),
            f"p={captured:,}",
            ha="center",
            va="bottom",
        )
    axis.set_xticks(positions, policy["modes"])
    axis.set_ylabel("Serialized bytes")
    axis.set_ylim(0, 12_800)
    axis.set_title("(b) Software log footprint", loc="left")
    axis.text(
        0.98,
        0.51,
        "6,272-B application trace",
        transform=axis.transAxes,
        ha="right",
        va="bottom",
    )
    axis.legend(loc="upper left", frameon=False, handlelength=1.5)
    style_axis(axis)

    axis = axes[1, 0]
    width = 0.34
    equivalent = policy["equivalent_us"]
    mismatch = policy["mismatch_us"]
    left = [value - width / 2 for value in positions]
    right = [value + width / 2 for value in positions]
    bars_equivalent = axis.bar(
        left,
        equivalent,
        width=width,
        color=BLUE,
        edgecolor=DARK,
        linewidth=0.55,
        label="Equivalent",
        zorder=3,
    )
    bars_mismatch = axis.bar(
        right,
        mismatch,
        width=width,
        color=GOLD,
        edgecolor=DARK,
        linewidth=0.55,
        hatch="///",
        label="Mismatch",
        zorder=3,
    )
    for bars in (bars_equivalent, bars_mismatch):
        for bar in bars:
            axis.text(
                bar.get_x() + bar.get_width() / 2,
                bar.get_height() + 0.5,
                f"{bar.get_height():.1f}",
                ha="center",
                va="bottom",
            )
    axis.set_xticks(positions, policy["modes"])
    axis.set_ylabel("Validator mean (µs)")
    axis.set_ylim(0, 29)
    axis.set_title("(c) Software validator time", loc="left")
    axis.text(
        0.98,
        0.05,
        "200 repetitions; dispersion not retained",
        transform=axis.transAxes,
        ha="right",
        va="bottom",
    )
    axis.legend(
        loc="upper left",
        frameon=False,
        ncol=2,
        columnspacing=0.8,
        handlelength=1.4,
    )
    style_axis(axis)

    axis = axes[1, 1]
    mechanisms = ("Framing", "Tag", "Count", "Dispatch", "Result")
    mechanism_index = {
        mechanism: index for index, mechanism in enumerate(mechanisms)
    }
    cases = data["offline_failstop"]
    matrix = [[0] * len(mechanisms) for _ in cases]
    for row_index, case in enumerate(cases):
        matrix[row_index][mechanism_index[case["mechanism"]]] = 1
    axis.imshow(
        matrix,
        cmap=ListedColormap([WHITE, BLUE]),
        vmin=0,
        vmax=1,
        interpolation="nearest",
        aspect="auto",
    )
    axis.set_xticks(
        range(len(mechanisms)),
        mechanisms,
        rotation=25,
        ha="right",
        rotation_mode="anchor",
    )
    axis.set_yticks(
        range(len(cases)),
        [case["mutation"] for case in cases],
    )
    axis.tick_params(axis="both", length=0)
    axis.set_xticks(
        [index - 0.5 for index in range(1, len(mechanisms))],
        minor=True,
    )
    axis.set_yticks(
        [index - 0.5 for index in range(1, len(cases))],
        minor=True,
    )
    axis.grid(which="minor", color=GRAY, linewidth=0.55)
    axis.tick_params(which="minor", bottom=False, left=False)
    for row_index, row in enumerate(matrix):
        for column_index, marked in enumerate(row):
            if marked:
                axis.text(
                    column_index,
                    row_index,
                    "●",
                    ha="center",
                    va="center",
                    color=WHITE,
                    fontweight="bold",
                )
    axis.set_title("(d) Offline fail-stop coverage", loc="left")
    axis.text(
        1.0,
        1.04,
        "7/7 detected",
        transform=axis.transAxes,
        ha="right",
        va="bottom",
        color=BLUE,
        fontweight="bold",
    )
    for spine in axis.spines.values():
        spine.set_color(DARK)
        spine.set_linewidth(0.6)

    output.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(output, format="pdf", metadata=PDF_METADATA)
    plt.close(fig)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Render SlugArch Figure 1")
    parser.add_argument("--data", type=Path, default=DEFAULT_DATA)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    render(args.data, args.output)
    print(f"wrote {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 6: Run the tests and render Figure 1**

Run:

```bash
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 -m unittest scripts/test_plot_slugarch_results.py -v
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 scripts/plot_slugarch_results.py
python3 scripts/check_paper_contract.py --stage legacy
```

Expected:

```text
test_render_is_deterministic_and_correct_size ... ok
test_reviewed_values ... ok
paper contract (legacy): PASS
```

- [ ] **Step 7: Inspect Figure 1 directly**

Run:

```bash
pdftocairo -png -singlefile -r 240 \
  img/slugarch-results.pdf /tmp/slugarch-results
```

Open `/tmp/slugarch-results.png`. Require:

- no clipped labels;
- all printed text at least 7 points;
- distinguishable markers/hatches in grayscale;
- the BAR2 one-boot boundary remains visible;
- panel (d) says offline and is not presented as live fault injection.

- [ ] **Step 8: Commit Figure 1 inputs, tests, renderer, and output**

Run:

```bash
git add \
  data/slugarch-results-20260704.json \
  scripts/test_plot_slugarch_results.py \
  scripts/plot_slugarch_results.py \
  img/slugarch-results.pdf
git diff --cached --check
git commit -m "fig: add four-panel SlugArch prototype evidence"
```

Expected: exactly four paths in the commit.

---

## Task 5: Compress the Non-Evaluation Narrative to Its Fixed Budget

**Files:**
- Modify: `intro.tex:1-42`
- Modify: `background.tex:1-63`
- Modify: `design.tex:38-130,132-159,200-290`
- Modify: `semantics.tex:271-480`
- Modify: `related.tex:1-121`

- [ ] **Step 1: Record the pre-compression counts**

Run:

```bash
for file in intro.tex background.tex design.tex semantics.tex related.tex; do
  printf '%-16s ' "$file"
  texcount -sum "$file" 2>/dev/null | tail -n 1
done
```

Expected: five baseline counts in the execution notes. Counts are diagnostic;
the PDF page-label gate, not a word-count target, determines completion.

- [ ] **Step 2: Replace the introduction with four compact units**

Replace `intro.tex` with:

```tex
\section{Introduction}

Heterogeneous systems increasingly move causality outside the CPU instruction
stream. A CPU submits work to GPUs, DPUs, memory controllers, and CXL devices,
while firmware, queues, translation state, coherence, and DMA determine when
memory effects become visible. CPU tracing and checkpoints can recover local
execution, but reconstructing a fabric-visible failure still requires
device-specific logs and coarse snapshots. The resulting \emph{causality gap}
is especially painful for intermittent ordering and memory-coherency bugs.

\sys makes the endpoint boundary the replay contract. Each endpoint remains a
black-box local machine, but a programmable \emc records the covered memory and
offload events that cross its boundary. A \hj specializes bounded controller
programs for the current ranges, queues, epochs, and policy. SlugArch replay
then validates or reissues those records in dependency order, accepting
private endpoint variation while requiring covered externally visible behavior
to match or fail-stop.

For a coherency failure, the workflow is concrete. First, select the shared
range, endpoints, and event classes. Second, record ownership transitions,
fences, writes, invalidations, and policy changes at each covered boundary.
Third, replay while perturbing one ordering edge or delay at a time. Fourth,
compare the earliest boundary divergence with the native execution. This
localizes a fabric-visible cause without reverse-engineering a proprietary
scheduler or cache implementation. Events that bypass the installed boundary
remain explicitly outside the guarantee.

This paper makes four contributions:
\begin{enumerate}[leftmargin=*]
  \item a programmable boundary replay contract for heterogeneous CXL systems;
  \item a bounded \hj/\emc policy model for recording, perturbation, and
  fail-stop validation;
  \item an operational semantics and boundary-equivalence theorem; and
  \item a scoped artifact showing guest-visible Type-2 BAR2 integration,
  software record tradeoffs, and, subject to the complete-campaign gate,
  calibrated Type-2 CXL.mem behavior.
\end{enumerate}
```

- [ ] **Step 3: Replace the background with three focused subsections**

Replace `background.tex` with:

```tex
\section{Background and Motivation}

\subsection{CPU replay and residual flow}

Processor tracing and deterministic replay can capture control flow,
interrupts, and selected memory behavior at comparatively low cost
\cite{intelpt,coresight,revirt,flashback,pinplay,rr}. Heterogeneous offload
moves essential decisions elsewhere: runtimes submit queues, devices schedule
work, IOMMUs translate addresses, and firmware or coherence agents determine
visibility. Checkpointing and accelerator-runtime systems recover useful state
\cite{dmtcp,remus,checl,crum,crac}, but their API- or page-level view may expose
an effect only after the boundary event that caused it.

\subsection{The causality gap}

We call the unrecorded device and fabric behavior \emph{residual flow}. It
includes queue consumption, DMA, migration, translation invalidation, cache
ownership transfer, CXL.mem access, MMIO input, and completion ordering. A
software log can state that a kernel was launched or a page became dirty
without identifying which boundary-visible operation made a later load stale.
The gap is therefore not another instruction-set problem: it is the absence of
a portable record of covered interactions among independently executing
components.

\subsection{CXL as the proposed boundary}

CXL.io provides discovery and control, CXL.mem exposes device-attached memory,
and CXL.cache permits coherent device caching \cite{cxl30}. Switching and
pooling further separate execution from memory placement
\cite{directcxl,pond,tmo}. SlugArch treats these interfaces as potential
observation points rather than assuming that current devices already implement
a replay plane. The present artifact evaluates only the named QEMU/CXLMemSim
paths. In particular, successful BAR2 or CXL.mem activity does not establish
CXL.cache, DMA, ATS, migration, switch ordering, or hardware replay.
```

- [ ] **Step 4: Replace repeated design policy prose with the fixed compact blocks**

In `design.tex`, retain `Architectural model`, `Assumptions and non-goals`,
`Program model and verification`, `Cross-protocol ordering`, and all four
numbered steps under `Debugging memory coherency`.

Replace the body of `\subsection{Coverage policy}` with:

```tex
A coverage policy names address ranges, queues, translation contexts,
protection domains, endpoints, event classes, and epochs, and assigns each
object full, delta, validation, or ordering-only recording. The policy and its
digest define the claim: divergence inside the covered set fails replay;
unsupported or bypassing event sources block the claim. Policies compose
conservatively, so stronger capture dominates weaker capture. This explicit
boundary prevents a BAR2 or CXL.mem integration result from being interpreted
as evidence for CXL.cache, DMA, ATS, migration, or switch ordering.
```

Replace the Hardware JIT input/output catalog with:

```tex
The input combines topology, protection state, workload hints, replay goals,
and an explicit metadata budget; the output is a bounded, verified controller
program and versioned policy digest. Programs classify covered transactions,
attach epochs and labels, choose the recording mode, and emit records. The
verifier rejects unbounded loops, out-of-range accesses, unsupported events,
and a policy whose worst-case record rate exceeds its declared budget.
```

Replace the replay-record field catalog with:

```tex
Each record carries identity, endpoint and epoch, event and object class,
ordering dependencies, recording mode, payload or commitment, labels, and the
policy digest. Full records retain values, delta records retain changes,
validation records retain commitments, and ordering-only records retain the
dependency edge. A sealed epoch binds the records, installed policy, and
unsupported-event list.
```

Replace the separate `Security and observability`, `Log sealing and trust
boundaries`, and `Compatibility path` bodies with:

```tex
\subsection{Trust and compatibility}

SlugArch trusts the installed controller, policy loader, seal verification,
and checkpoint root. An endpoint that bypasses the controller, forges labels,
or changes records after sealing is outside the guarantee; the deployment must
report that gap rather than accept replay. IOMMU policy, firmware measurement,
authenticated logs, and revocation complement this boundary but are not
implemented by the present artifact.

Existing devices can first expose a software- or firmware-assisted boundary,
then move record generation into an endpoint or switch controller. Each stage
uses the same event and policy contract but states which events still depend on
trusted software. Compatibility is therefore incremental, not evidence that a
legacy endpoint already provides transparent fabric-wide replay.
```

- [ ] **Step 5: Group semantic obligations and reduce correctness to one theorem**

Replace `semantics.tex` from `\subsection{Corner cases and obligations}` through
the end of the file with:

```tex
\subsection{Obligations and corner cases}

The boundary theorem depends on four grouped obligations. \emph{Memory
ordering} covers concurrent races, atomics, weak memory, fences, out-of-order
fabric delivery, write combining, and partial writes: the log must contain
every covered predecessor needed to preserve visibility, and replay may choose
any linear extension of that relation. \emph{Address and ownership} covers
DMA, peer-to-peer traffic, migration, ATS/IOTLB invalidation, device-private
memory, and protection labels; a private effect that later influences a
covered event must be represented at the boundary. \emph{Nondeterminism}
covers MMIO, external input, timers, interrupts, poison, retry, reset, and
failure; values or ordering that can affect a covered event require full
records or an explicit device model. \emph{Log integrity} requires unique
records, satisfied dependencies, compatible event contracts,
collision-resistant commitments, valid seals, and fail-stop handling of
truncation, duplication, corruption, or a stall that cannot make progress.
Unsupported obligations are reported as coverage failures rather than
accepted replay.

\subsection{Correctness}

\begin{theorem}[Boundary replay correctness]
Let a native execution start from checkpoint $C$ and reach sealed state $S$
while producing a well-formed log $L$ under coverage policy $\mathcal{C}$. If
every claimed boundary event crosses an installed \emc, each record contains
all covered visibility-affecting predecessors, validation commitments are
collision resistant, nondeterministic covered values are recorded, and replay
implements the same boundary event and label contract, then every successful
\sys replay that consumes exactly $L$ reaches $S_R$ with
$S \equiv_{\mathcal{C}} S_R$.
\end{theorem}

\begin{proof}[Proof sketch]
Order $L$ topologically by its dependency relation. The checkpoint establishes
the base boundary equivalence. For an enabled next record, recording coverage
provides a matching native event and all covered predecessors; replay stalls an
early event, consumes only a matching enabled event, and rejects an unmatched
event. Each successful step therefore preserves boundary equivalence for
memory, ownership, queue, translation, protection, payload, and label state.
After the log is consumed, the epoch seal establishes agreement for every
covered object. A missing event, bad commitment, invalid seal, incompatible
label, or unsatisfiable dependency instead produces explicit divergence.
Hence replay cannot silently commit different covered behavior.
\end{proof}

The theorem does not require cycle-identical execution or reproduction of
private scheduling and replacement decisions. It requires ordered covered
behavior to match or fail-stop before an incompatible boundary event commits.
```

- [ ] **Step 6: Replace related work and the closing sections**

Replace `related.tex` with:

```tex
\section{Related Work}

\paragraph{Processor tracing and replay.}
Intel Processor Trace, Arm CoreSight, deterministic replay, and checkpointing
capture CPU- and software-visible execution \cite{intelpt,coresight,revirt,
flashback,pinplay,rr,dmtcp,remus}. SlugArch instead defines the covered device
and memory-fabric boundary events that those traces cannot reconstruct.

\paragraph{Accelerator state.}
GPU checkpointing and unified-memory systems recover accelerator processes and
managed state \cite{nvidiauvm,checl,crum,crac}. They are complementary:
runtime hints can name semantic objects while SlugArch checks the corresponding
covered boundary behavior.

\paragraph{CXL systems.}
DirectCXL, Pond, and TMO use CXL for direct access, pooling, and memory
offloading \cite{directcxl,pond,tmo}. SlugArch uses the fabric as a proposed
record and validation boundary; it does not claim that current CXL hardware
already exports the required replay hooks.

\paragraph{Programmable memory and provenance.}
Near-memory systems and programmable controllers place computation close to
data \cite{tesseract,mondrian,xrp,bpf}, while provenance systems audit
information flow \cite{camflow}. SlugArch specializes bounded programs for
recording and validation, with a policy digest that exposes which events and
objects the result actually covers.

\section{Deployment and Limitations}

SlugArch can begin as the simulator-assisted path evaluated here, progress to
firmware- or runtime-assisted endpoint hooks, and ultimately move policy
loading, record emission, and sealing into standardized endpoint and switch
capabilities. Each stage must name which events still depend on trusted
software. The guarantee excludes an endpoint that bypasses its controller,
forges labels, or modifies records after sealing; deployment therefore also
requires IOMMU policy, firmware measurement, authenticated logs, revocation,
and explicit behavior under metadata backpressure. Full capture may be
necessary for nondeterministic MMIO, failure, or untrusted DMA, so the
fidelity--cost tradeoff remains visible in the installed policy and artifact.

\section{Conclusion}

SlugArch makes replay a boundary contract for heterogeneous memory fabrics.
Its formal model requires covered events to match or fail-stop. The current
artifact demonstrates only the legacy BAR2 boundary and, after the complete
campaign gate passes, one server-authoritative QEMU/CXLMemSim Type-2 CXL.mem
path. Device-side replay, CXL.cache, DMA, ATS, migration, switch ordering,
recovery, and post-fit hardware cost remain future measurements rather than
conclusions from simulator activity.
```

- [ ] **Step 7: Build and verify the compressed sources**

Run:

```bash
latexmk -pdf -interaction=nonstopmode -halt-on-error main.tex
python3 scripts/check_paper_contract.py --stage legacy
rg -n \
  'Undefined control sequence|LaTeX Error|undefined references|undefined citations|Citation .* undefined|Reference .* undefined' \
  main.log
```

Expected:

```text
paper contract (legacy): PASS
```

The warning search must be empty.

- [ ] **Step 8: Commit the narrative compression**

Run:

```bash
git add intro.tex background.tex design.tex semantics.tex related.tex
git diff --cached --check
git commit -m "paper: compress SlugArch design and correctness narrative"
```

Expected: only the five narrative files are committed.

---

## Task 6: Define and Implement the Figure 2 Data Contract

**Files:**
- Create: `scripts/test_plot_slugarch_type2_cxlmem.py`
- Create: `scripts/plot_slugarch_type2_cxlmem.py`
- Do not create yet: `data/slugarch-type2-cxlmem.json`
- Do not create yet: `img/slugarch-type2-cxlmem.pdf`

The normalized export consumed by this task has this exact shape:

```text
schema_version: 1
campaign:
  campaign_id: nonempty string
  experiment_version_sha256: 64 lowercase hexadecimal characters
  campaign_checksum_sha256: 64 lowercase hexadecimal characters
  registry_ordinal: positive integer
  artifact_relative_path: artifact/slugarch_type2_cxlmem/slugarch-type2-cxlmem-v1-20260724
  complete: true
  eligible: true
  committed_boots: 20
  latencies_ns: [80, 400, 2000, 10000]
  repeats_per_latency: 5
  warmup_passes_per_boot: 1
  timed_passes_per_boot: 1
  corruption_rejections: 20
panel_gates:
  protocol: true
  sentinel: true
  bypass: true
  artifact: true
  corruption: true
  calibration: true
  transfer: true
  paired_overhead: true
  record_scaling: true
validation:
  source_hashes_match: true
  protocol_counts_match: true
  byte_totals_match: true
  delay_event_join_complete: true
  zero_bypass_completions: true
  phase_barriers_valid: true
  no_failed_attempts_included: true
calibration: 20 boot-level rows
transfer: 60 boot/size rows
slugarch: 320 boot/request-record-count/mode rows
corruption: 20 boot-level rows
claim_limitations: nonempty string list
provenance: source hashes, registry history, raw relative paths, exclusions,
  and blocking gate codes when ineligible
```

Each observation row carries `latency_ns`, `replicate`, `attempt_id`,
`guest_boot_uuid`, and `server_instance_uuid`. Calibration rows additionally
carry `load_count`,
`read_bytes`, `elapsed_ns`, and `ns_per_load`. Transfer rows carry
`size_bytes`, `write_ns`, `read_ns`, `write_bytes`, `read_bytes`,
`write_gib_s`, and `read_gib_s`. SlugArch rows carry `copy_count`,
`request_record_count`, `response_record_count`, `boundary_record_count`,
`mode`, `end_to_end_ns`, `baseline_end_to_end_ns`, and `paired_overhead`.
Corruption rows carry `test_name`, `rejected`, and `signal`.

- [ ] **Step 1: Write the Figure 2 tests before the renderer**

Create `scripts/test_plot_slugarch_type2_cxlmem.py`:

```python
#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from plot_slugarch_type2_cxlmem import (  # noqa: E402
    load_and_validate,
    render,
    render_mockup,
)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class Type2FigureTests(unittest.TestCase):
    def test_mockup_is_deterministic_and_correct_size(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            first = Path(directory) / "first.pdf"
            second = Path(directory) / "second.pdf"
            render_mockup(first)
            render_mockup(second)
            self.assertEqual(digest(first), digest(second))
            info = subprocess.run(
                ("pdfinfo", str(first)),
                check=True,
                text=True,
                capture_output=True,
            ).stdout
            self.assertIn("Pages:           1", info)
            self.assertRegex(info, r"Page size:\s+511\.2 x 216 pts")

    def test_complete_snapshot_and_render(self) -> None:
        data_path = ROOT / "data" / "slugarch-type2-cxlmem.json"
        self.assertTrue(
            data_path.exists(),
            "complete-campaign paper snapshot is required",
        )
        data = load_and_validate(data_path)
        self.assertEqual(data["campaign"]["committed_boots"], 20)
        self.assertEqual(data["campaign"]["corruption_rejections"], 20)
        with tempfile.TemporaryDirectory() as directory:
            first = Path(directory) / "first.pdf"
            second = Path(directory) / "second.pdf"
            render(data_path, first)
            render(data_path, second)
            self.assertEqual(digest(first), digest(second))


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the mockup test and verify the missing-module failure**

Run:

```bash
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 -m unittest \
  scripts.test_plot_slugarch_type2_cxlmem.Type2FigureTests.test_mockup_is_deterministic_and_correct_size \
  -v
```

Expected: FAIL with `ModuleNotFoundError: No module named
'plot_slugarch_type2_cxlmem'`.

- [ ] **Step 3: Implement the validated Figure 2 renderer**

Create `scripts/plot_slugarch_type2_cxlmem.py`:

```python
#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import math
import re
import statistics
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable

import matplotlib

matplotlib.use("pdf")
import matplotlib.pyplot as plt  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DATA = ROOT / "data" / "slugarch-type2-cxlmem.json"
DEFAULT_OUTPUT = ROOT / "img" / "slugarch-type2-cxlmem.pdf"

LATENCIES = (80, 400, 2000, 10000)
REPLICATES = (1, 2, 3, 4, 5)
SIZES = (4096, 65536, 1048576)
REQUEST_RECORD_COUNTS = (49, 196, 784, 3136)
MODES = ("baseline", "validation", "delta", "full")
RATIO_MODES = ("validation", "delta", "full")
GATES = (
    "protocol",
    "sentinel",
    "bypass",
    "artifact",
    "corruption",
    "calibration",
    "transfer",
    "paired_overhead",
    "record_scaling",
)
VALIDATION_FLAGS = (
    "source_hashes_match",
    "protocol_counts_match",
    "byte_totals_match",
    "delay_event_join_complete",
    "zero_bypass_completions",
    "phase_barriers_valid",
    "no_failed_attempts_included",
)
LATENCY_LABELS = ("80 ns", "400 ns", "2 µs", "10 µs")
LATENCY_COLORS = ("#0b559f", "#2b7bba", "#69a7cf", "#b7d4e8")
MODE_STYLE = {
    "baseline": ("#17212b", "o", "-"),
    "validation": ("#2878b5", "s", "-"),
    "delta": ("#d99b28", "^", "--"),
    "full": ("#8b5a9f", "D", ":"),
}
DARK = "#17212b"
GRAY = "#d9dee3"
WHITE = "#ffffff"
HEX64 = re.compile(r"^[0-9a-f]{64}$")
PDF_METADATA = {
    "Title": "SlugArch Type-2 CXL.mem calibration and evaluation",
    "Author": "Anonymous",
    "Subject": "Validated QEMU and CXLMemSim simulator measurements",
    "Keywords": "SlugArch, Type-2, CXL.mem, CXLMemSim",
    "Creator": "SlugArch deterministic figure renderer",
    "Producer": "Matplotlib",
    "CreationDate": datetime(2026, 7, 24, tzinfo=timezone.utc),
    "ModDate": datetime(2026, 7, 24, tzinfo=timezone.utc),
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def configure() -> None:
    matplotlib.rcParams.update(
        {
            "font.family": "DejaVu Sans",
            "font.size": 7.0,
            "axes.titlesize": 7.2,
            "axes.labelsize": 7.0,
            "xtick.labelsize": 7.0,
            "ytick.labelsize": 7.0,
            "legend.fontsize": 7.0,
            "pdf.fonttype": 42,
            "ps.fonttype": 42,
            "axes.edgecolor": DARK,
            "axes.labelcolor": DARK,
            "text.color": DARK,
            "xtick.color": DARK,
            "ytick.color": DARK,
        }
    )


def style_axis(axis: plt.Axes) -> None:
    axis.grid(axis="y", color=GRAY, linewidth=0.55, zorder=0)
    axis.spines["top"].set_visible(False)
    axis.spines["right"].set_visible(False)
    axis.spines["left"].set_linewidth(0.6)
    axis.spines["bottom"].set_linewidth(0.6)


def median_min_max(values: Iterable[float]) -> tuple[float, float, float]:
    sequence = [float(value) for value in values]
    require(len(sequence) == 5, "each plotted cell must contain five boots")
    return statistics.median(sequence), min(sequence), max(sequence)


def unique_slots(rows: list[dict]) -> set[tuple[int, int]]:
    return {
        (int(row["latency_ns"]), int(row["replicate"]))
        for row in rows
    }


def validate_campaign(data: dict) -> None:
    campaign = data["campaign"]
    require(data["schema_version"] == 1, "unsupported Type-2 schema")
    require(campaign["complete"] is True, "campaign is not complete")
    require(campaign["eligible"] is True, "campaign is not paper-eligible")
    require(campaign["committed_boots"] == 20, "expected 20 committed boots")
    require(
        campaign["latencies_ns"] == list(LATENCIES),
        "latency grid drifted",
    )
    require(campaign["repeats_per_latency"] == 5, "repeat count drifted")
    require(
        campaign["warmup_passes_per_boot"] == 1,
        "warmup count drifted",
    )
    require(
        campaign["timed_passes_per_boot"] == 1,
        "timed-pass count drifted",
    )
    require(
        campaign["corruption_rejections"] == 20,
        "expected 20 corruption rejections",
    )
    require(
        HEX64.fullmatch(campaign["experiment_version_sha256"]) is not None,
        "bad experiment-version hash",
    )
    require(
        HEX64.fullmatch(campaign["campaign_checksum_sha256"]) is not None,
        "bad campaign checksum hash",
    )
    require(campaign["registry_ordinal"] > 0, "bad registry ordinal")
    require(
        campaign["artifact_relative_path"].startswith(
            "artifact/slugarch_type2_cxlmem/"
        ),
        "bad campaign artifact path",
    )
    require(
        set(data["panel_gates"]) == set(GATES),
        "panel-gate set drifted",
    )
    require(
        all(data["panel_gates"][name] is True for name in GATES),
        "one or more Figure 2 gates failed",
    )
    require(
        set(data["validation"]) == set(VALIDATION_FLAGS),
        "validation-flag set drifted",
    )
    require(
        all(data["validation"][name] is True for name in VALIDATION_FLAGS),
        "one or more campaign data-quality checks failed",
    )
    require(
        bool(data["claim_limitations"]),
        "claim limitations are missing",
    )
    require(bool(data["provenance"]), "provenance is missing")


def validate_rows(data: dict) -> None:
    expected_slots = {
        (latency, replicate)
        for latency in LATENCIES
        for replicate in REPLICATES
    }

    calibration = data["calibration"]
    require(len(calibration) == 20, "expected 20 calibration rows")
    require(unique_slots(calibration) == expected_slots, "calibration slots drifted")
    for row in calibration:
        require(row["load_count"] == 4096, "calibration load count drifted")
        require(row["read_bytes"] == 32768, "calibration byte count drifted")
        require(row["elapsed_ns"] > 0, "nonpositive calibration duration")
        require(row["ns_per_load"] > 0, "nonpositive load latency")
        require(bool(row["guest_boot_uuid"]), "missing calibration boot UUID")

    transfer = data["transfer"]
    require(len(transfer) == 60, "expected 60 transfer rows")
    transfer_keys = {
        (row["latency_ns"], row["replicate"], row["size_bytes"])
        for row in transfer
    }
    expected_transfer_keys = {
        (latency, replicate, size)
        for latency in LATENCIES
        for replicate in REPLICATES
        for size in SIZES
    }
    require(transfer_keys == expected_transfer_keys, "transfer matrix drifted")
    for row in transfer:
        require(row["write_bytes"] == row["size_bytes"], "write bytes drifted")
        require(row["read_bytes"] == row["size_bytes"], "read bytes drifted")
        require(row["write_ns"] > 0 and row["read_ns"] > 0, "bad transfer time")
        require(
            row["write_gib_s"] > 0 and row["read_gib_s"] > 0,
            "bad transfer throughput",
        )
        require(bool(row["guest_boot_uuid"]), "missing transfer boot UUID")

    slugarch = data["slugarch"]
    require(len(slugarch) == 320, "expected 320 SlugArch rows")
    slugarch_keys = {
        (
            row["latency_ns"],
            row["replicate"],
            row["request_record_count"],
            row["mode"],
        )
        for row in slugarch
    }
    expected_slugarch_keys = {
        (latency, replicate, request_record_count, mode)
        for latency in LATENCIES
        for replicate in REPLICATES
        for request_record_count in REQUEST_RECORD_COUNTS
        for mode in MODES
    }
    require(slugarch_keys == expected_slugarch_keys, "SlugArch matrix drifted")
    for row in slugarch:
        require(
            row["response_record_count"] == row["request_record_count"],
            "response/request record counts differ",
        )
        require(
            row["boundary_record_count"]
            == 2 * row["request_record_count"],
            "boundary record count is not twice request count",
        )
        require(
            row["copy_count"] * 49 == row["request_record_count"],
            "copy count and request record count differ",
        )
        require(row["end_to_end_ns"] > 0, "bad SlugArch duration")
        require(row["baseline_end_to_end_ns"] > 0, "bad paired baseline")
        require(bool(row["guest_boot_uuid"]), "missing SlugArch boot UUID")
        expected_ratio = (
            row["end_to_end_ns"] / row["baseline_end_to_end_ns"]
        )
        require(
            math.isclose(
                row["paired_overhead"],
                expected_ratio,
                rel_tol=1e-12,
                abs_tol=1e-12,
            ),
            "paired overhead is not the same-boot ratio",
        )
        if row["mode"] == "baseline":
            require(
                math.isclose(row["paired_overhead"], 1.0),
                "baseline ratio is not one",
            )

    corruption = data["corruption"]
    require(len(corruption) == 20, "expected 20 corruption rows")
    require(unique_slots(corruption) == expected_slots, "corruption slots drifted")
    for row in corruption:
        require(
            row["test_name"] == "post_transport_guest_payload_flip",
            "corruption test name drifted",
        )
        require(row["rejected"] is True, "corrupted stream was accepted")
        require(
            row["signal"] == "decoded_result_mismatch",
            "unexpected corruption rejection signal",
        )

    identities: dict[tuple[int, int], set[tuple[str, str, str]]] = defaultdict(set)
    for collection in (calibration, transfer, slugarch, corruption):
        for row in collection:
            identities[(row["latency_ns"], row["replicate"])].add(
                (
                    row["attempt_id"],
                    row["guest_boot_uuid"],
                    row["server_instance_uuid"],
                )
            )
    require(
        set(identities) == expected_slots,
        "identity slot set drifted",
    )
    require(
        all(len(values) == 1 for values in identities.values()),
        "one replicate slot contains multiple attempt identities",
    )
    committed_identities = [next(iter(values)) for values in identities.values()]
    require(
        len({value[0] for value in committed_identities}) == 20,
        "committed attempt IDs are not globally unique",
    )
    require(
        len({value[1] for value in committed_identities}) == 20,
        "guest boot UUIDs are not globally unique",
    )
    require(
        len({value[2] for value in committed_identities}) == 20,
        "server instance UUIDs are not globally unique",
    )

    baselines = {
        (
            row["latency_ns"],
            row["replicate"],
            row["request_record_count"],
            row["guest_boot_uuid"],
        ): row["end_to_end_ns"]
        for row in slugarch
        if row["mode"] == "baseline"
    }
    for row in slugarch:
        key = (
            row["latency_ns"],
            row["replicate"],
            row["request_record_count"],
            row["guest_boot_uuid"],
        )
        require(key in baselines, "same-boot baseline row is missing")
        require(
            row["baseline_end_to_end_ns"] == baselines[key],
            "reported baseline does not match the same-boot baseline row",
        )


def validate_monotonicity(data: dict) -> None:
    calibration_medians = []
    for latency in LATENCIES:
        values = [
            row["ns_per_load"]
            for row in data["calibration"]
            if row["latency_ns"] == latency
        ]
        calibration_medians.append(statistics.median(values))
    require(
        calibration_medians == sorted(calibration_medians),
        "calibration medians are not nondecreasing",
    )
    require(
        calibration_medians[-1] > calibration_medians[0],
        "10 us calibration median is not greater than 80 ns",
    )

    for size in SIZES:
        for field in ("write_ns", "read_ns"):
            medians = []
            for latency in LATENCIES:
                values = [
                    row[field]
                    for row in data["transfer"]
                    if row["latency_ns"] == latency
                    and row["size_bytes"] == size
                ]
                medians.append(statistics.median(values))
            require(
                medians == sorted(medians),
                f"{field} medians are not nondecreasing for {size} bytes",
            )
            require(
                medians[-1] > medians[0],
                f"10 us {field} median is not greater than 80 ns for {size} bytes",
            )


def load_and_validate(path: Path) -> dict:
    data = json.loads(path.read_text(encoding="utf-8"))
    validate_campaign(data)
    validate_rows(data)
    validate_monotonicity(data)
    return data


def create_axes() -> tuple[
    plt.Figure,
    plt.Axes,
    plt.Axes,
    plt.Axes,
    plt.Axes,
    plt.Axes,
]:
    configure()
    fig = plt.figure(figsize=(7.1, 3.0))
    outer = fig.add_gridspec(
        2,
        2,
        left=0.075,
        right=0.988,
        bottom=0.155,
        top=0.91,
        wspace=0.31,
        hspace=0.62,
    )
    calibration_axis = fig.add_subplot(outer[0, 0])
    transfer_grid = outer[0, 1].subgridspec(1, 2, wspace=0.16)
    read_axis = fig.add_subplot(transfer_grid[0, 0])
    write_axis = fig.add_subplot(transfer_grid[0, 1], sharey=read_axis)
    overhead_axis = fig.add_subplot(outer[1, 0])
    scaling_axis = fig.add_subplot(outer[1, 1])
    return (
        fig,
        calibration_axis,
        read_axis,
        write_axis,
        overhead_axis,
        scaling_axis,
    )


def render_mockup(output: Path) -> None:
    (
        fig,
        calibration_axis,
        read_axis,
        write_axis,
        overhead_axis,
        scaling_axis,
    ) = create_axes()
    calibration_axis.set_title("(a) Configured-delay calibration", loc="left")
    calibration_axis.set_xlabel("Configured simulator latency")
    calibration_axis.set_ylabel("Guest ns / dependent load")
    calibration_axis.text(
        0.5,
        0.5,
        "LAYOUT ONLY — NO DATA",
        transform=calibration_axis.transAxes,
        ha="center",
        va="center",
        fontweight="bold",
    )

    read_axis.set_title("(b) Read", loc="left")
    write_axis.set_title("Write", loc="left")
    read_axis.set_xlabel("Transfer size")
    write_axis.set_xlabel("Transfer size")
    read_axis.set_ylabel("Guest-effective GiB/s")
    write_axis.tick_params(labelleft=False)
    for axis in (read_axis, write_axis):
        axis.text(
            0.5,
            0.5,
            "NO DATA",
            transform=axis.transAxes,
            ha="center",
            va="center",
            fontweight="bold",
        )

    overhead_axis.set_title("(c) Paired SlugArch overhead", loc="left")
    overhead_axis.set_xlabel("Configured simulator latency")
    overhead_axis.set_ylabel("Same-boot mode / baseline")
    overhead_axis.axhline(1.0, color=DARK, linewidth=0.7, linestyle=(0, (3, 2)))
    overhead_axis.text(
        0.5,
        0.5,
        "LAYOUT ONLY — NO DATA",
        transform=overhead_axis.transAxes,
        ha="center",
        va="center",
        fontweight="bold",
    )

    scaling_axis.set_title("(d) Request-record scaling at 400 ns", loc="left")
    scaling_axis.set_xlabel("Repeated-trace request records")
    scaling_axis.set_ylabel("End-to-end time")
    scaling_axis.text(
        0.5,
        0.5,
        "LAYOUT ONLY — NO DATA",
        transform=scaling_axis.transAxes,
        ha="center",
        va="center",
        fontweight="bold",
    )
    for axis in (
        calibration_axis,
        read_axis,
        write_axis,
        overhead_axis,
        scaling_axis,
    ):
        style_axis(axis)

    output.parent.mkdir(parents=True, exist_ok=True)
    metadata = dict(PDF_METADATA)
    metadata["Title"] = "SlugArch Figure 2 layout mockup without data"
    fig.savefig(output, format="pdf", metadata=metadata)
    plt.close(fig)


def plot_summary(
    axis: plt.Axes,
    x_value: float,
    values: list[float],
    color: str,
    marker: str,
    offset: float = 0.0,
) -> None:
    median, minimum, maximum = median_min_max(values)
    jitter = (-0.075, -0.0375, 0.0, 0.0375, 0.075)
    axis.scatter(
        [x_value + offset + value for value in jitter],
        values,
        s=10,
        facecolor=WHITE,
        edgecolor=color,
        linewidth=0.55,
        marker=marker,
        zorder=3,
    )
    axis.errorbar(
        x_value + offset,
        median,
        yerr=[[median - minimum], [maximum - median]],
        color=color,
        marker=marker,
        markerfacecolor=color,
        markeredgecolor=DARK,
        markeredgewidth=0.4,
        markersize=3.5,
        capsize=2.0,
        linewidth=0.8,
        zorder=4,
    )


def render(data_path: Path, output: Path) -> None:
    data = load_and_validate(data_path)
    (
        fig,
        calibration_axis,
        read_axis,
        write_axis,
        overhead_axis,
        scaling_axis,
    ) = create_axes()
    latency_positions = list(range(len(LATENCIES)))

    for position, latency, color in zip(
        latency_positions,
        LATENCIES,
        LATENCY_COLORS,
    ):
        values = [
            row["ns_per_load"]
            for row in data["calibration"]
            if row["latency_ns"] == latency
        ]
        plot_summary(calibration_axis, position, values, color, "o")
    calibration_axis.set_xticks(latency_positions, LATENCY_LABELS)
    calibration_axis.set_yscale("log")
    calibration_axis.set_xlabel("Configured simulator latency")
    calibration_axis.set_ylabel("Guest ns / dependent load")
    calibration_axis.set_title("(a) Configured-delay calibration", loc="left")
    style_axis(calibration_axis)

    size_positions = list(range(len(SIZES)))
    size_labels = ("4 KiB", "64 KiB", "1 MiB")
    latency_offsets = (-0.18, -0.06, 0.06, 0.18)
    for axis, field, title in (
        (read_axis, "read_gib_s", "(b) Read"),
        (write_axis, "write_gib_s", "Write"),
    ):
        for latency, color, offset, latency_label in zip(
            LATENCIES,
            LATENCY_COLORS,
            latency_offsets,
            LATENCY_LABELS,
        ):
            medians = []
            for position, size in zip(size_positions, SIZES):
                values = [
                    row[field]
                    for row in data["transfer"]
                    if row["latency_ns"] == latency
                    and row["size_bytes"] == size
                ]
                plot_summary(axis, position, values, color, "o", offset)
                medians.append(statistics.median(values))
            axis.plot(
                [position + offset for position in size_positions],
                medians,
                color=color,
                linewidth=0.8,
                label=latency_label,
                zorder=2,
            )
        axis.set_xticks(size_positions, size_labels, rotation=20, ha="right")
        axis.set_yscale("log")
        axis.set_xlabel("Transfer size")
        axis.set_title(title, loc="left")
        style_axis(axis)
    read_axis.set_ylabel("Guest-effective GiB/s")
    write_axis.tick_params(labelleft=False)
    handles, labels = write_axis.get_legend_handles_labels()
    fig.legend(
        handles,
        labels,
        loc="upper right",
        bbox_to_anchor=(0.988, 0.995),
        frameon=False,
        ncol=4,
        columnspacing=0.7,
        handlelength=1.2,
    )

    overhead_offsets = (-0.10, 0.0, 0.10)
    for mode, offset in zip(RATIO_MODES, overhead_offsets):
        color, marker, line_style = MODE_STYLE[mode]
        medians = []
        for position, latency in zip(latency_positions, LATENCIES):
            values = [
                row["paired_overhead"]
                for row in data["slugarch"]
                if row["latency_ns"] == latency
                and row["request_record_count"] == 196
                and row["mode"] == mode
            ]
            plot_summary(overhead_axis, position, values, color, marker, offset)
            medians.append(statistics.median(values))
        overhead_axis.plot(
            [position + offset for position in latency_positions],
            medians,
            color=color,
            marker=marker,
            markersize=3.0,
            linewidth=0.8,
            linestyle=line_style,
            label=mode.capitalize(),
            zorder=2,
        )
    overhead_axis.axhline(
        1.0,
        color=DARK,
        linewidth=0.7,
        linestyle=(0, (3, 2)),
        zorder=1,
    )
    overhead_axis.set_xticks(latency_positions, LATENCY_LABELS)
    overhead_axis.set_xlabel("Configured simulator latency")
    overhead_axis.set_ylabel("Same-boot mode / baseline")
    overhead_axis.set_title(
        "(c) Paired overhead, 196 request records",
        loc="left",
    )
    overhead_axis.legend(
        loc="upper left",
        frameon=False,
        ncol=3,
        columnspacing=0.7,
        handlelength=1.3,
    )
    style_axis(overhead_axis)

    record_positions = list(range(len(REQUEST_RECORD_COUNTS)))
    record_labels = ("49", "196", "784", "3,136")
    for mode in MODES:
        color, marker, line_style = MODE_STYLE[mode]
        medians = []
        minimums = []
        maximums = []
        for request_record_count in REQUEST_RECORD_COUNTS:
            values = [
                row["end_to_end_ns"] / 1_000_000.0
                for row in data["slugarch"]
                if row["latency_ns"] == 400
                and row["request_record_count"] == request_record_count
                and row["mode"] == mode
            ]
            median, minimum, maximum = median_min_max(values)
            medians.append(median)
            minimums.append(minimum)
            maximums.append(maximum)
        scaling_axis.plot(
            record_positions,
            medians,
            color=color,
            marker=marker,
            markersize=3.2,
            linewidth=0.85,
            linestyle=line_style,
            label=mode.capitalize(),
            zorder=3,
        )
        scaling_axis.fill_between(
            record_positions,
            minimums,
            maximums,
            color=color,
            alpha=0.10,
            linewidth=0,
            zorder=1,
        )
    scaling_axis.set_xticks(record_positions, record_labels)
    scaling_axis.set_yscale("log")
    scaling_axis.set_xlabel("Repeated-trace request records")
    scaling_axis.set_ylabel("End-to-end time (ms)")
    scaling_axis.set_title("(d) Record scaling at 400 ns", loc="left")
    scaling_axis.legend(
        loc="upper left",
        frameon=False,
        ncol=2,
        columnspacing=0.8,
        handlelength=1.4,
    )
    style_axis(scaling_axis)

    output.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(output, format="pdf", metadata=PDF_METADATA)
    plt.close(fig)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Render SlugArch Figure 2")
    parser.add_argument("--data", type=Path, default=DEFAULT_DATA)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--mockup", action="store_true")
    parser.add_argument("--check-only", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    require(
        not (args.mockup and args.check_only),
        "--mockup and --check-only are mutually exclusive",
    )
    if args.mockup:
        render_mockup(args.output)
        print(f"wrote layout-only mockup {args.output}")
        return 0
    load_and_validate(args.data)
    if args.check_only:
        print("Type-2 paper snapshot: PASS")
        return 0
    render(args.data, args.output)
    print(f"wrote {args.output}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (KeyError, ValueError, json.JSONDecodeError) as exc:
        print(f"Type-2 paper snapshot: FAIL: {exc}")
        raise SystemExit(1)
```

- [ ] **Step 4: Run only the mockup test**

Run:

```bash
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 -m unittest \
  scripts.test_plot_slugarch_type2_cxlmem.Type2FigureTests.test_mockup_is_deterministic_and_correct_size \
  -v
```

Expected:

```text
test_mockup_is_deterministic_and_correct_size ... ok
```

Do not run `test_complete_snapshot_and_render` yet: its intentional missing-data
failure is the complete-campaign gate used in Task 8.

- [ ] **Step 5: Commit the Figure 2 contract and renderer without data**

Run:

```bash
git add \
  scripts/test_plot_slugarch_type2_cxlmem.py \
  scripts/plot_slugarch_type2_cxlmem.py
git diff --cached --check
git commit -m "test: define complete Type-2 figure contract"
```

Expected: two script files in the commit and no Type-2 JSON or figure.

---

## Task 7: Prove the Two-Figure, Three-Page Layout Before the Campaign

**Files:**
- Create: `scripts/build_slugarch_layout_mockup.py`
- Generate only under `/tmp`: `/tmp/slugarch-paper-layout`

This task must run before the 20-slot campaign. It uses Figure 1's reviewed
values and an explicitly empty Figure 2. It never writes a mockup into the
paper repository and never commits a numeric preview.

- [ ] **Step 1: Run the absent mockup command to establish the failing gate**

Run:

```bash
python3 scripts/build_slugarch_layout_mockup.py
```

Expected: FAIL because `scripts/build_slugarch_layout_mockup.py` does not yet
exist.

- [ ] **Step 2: Implement the isolated layout builder**

Create `scripts/build_slugarch_layout_mockup.py`:

```python
#!/usr/bin/env python3
from __future__ import annotations

import re
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEST = Path("/tmp/slugarch-paper-layout")
COPY = DEST / "paper"

MOCK_EVALUATION = r"""
\section{Evaluation}
\label{sec:evaluation}

We separate the existing BAR2/software evidence from the new Type-2 CXL.mem
experiment. Figure~\ref{fig:slugarch-results} uses the reviewed July artifacts.
Figure~\ref{fig:slugarch-type2-cxlmem} is an empty layout mockup in this build:
it fixes axes, legends, caption space, and reading order without preview or
estimated numbers.

\subsection{Setup, artifact policy, and evidence boundary}

The legacy pass used one QEMU TCG guest boot and a Type-2 BAR2 command window.
Each request was written to and read from BAR2 before a NOP completed; guest
software generated the response stream. The new experiment instead requires a
mapped CXL fixed memory window, a devdax work range outside both local RAM
overlays, synchronous server-authoritative memory, and matching guest, QEMU,
and CXLMemSim counts. Only a complete, checksum-valid 20-boot campaign may
replace this mockup.

\subsection{Existing prototype evidence}

\begin{figure*}[t]
  \centering
  \includegraphics[width=\textwidth]{img/slugarch-results.pdf}
  \caption{Existing SlugArch prototype evidence. (a) Five helper runs in one
  TCG guest boot; BAR2 carried request write/readback and NOP while guest
  software generated responses. (b) Software log bytes. (c) Means of 200
  validator repetitions; dispersion was not retained. (d) Seven offline
  response-stream mutations, all rejected.}
  \label{fig:slugarch-results}
\end{figure*}

Figure~\ref{fig:slugarch-results}(a) presents the five back-to-back helper
runs as integration repeatability, not independent boots or physical-link
latency. Panels (b) and (c) report software record-format and validator
behavior. Panel (d) reports offline mutations rather than device injection or
recovery. No response FLIT traverses BAR2.

\subsection{Type-2 CXL.mem calibration}

The new path uses four configured simulator settings and five complete guest
boots per setting. Every committed boot performs one untimed warmup pass and
one timed pass. A 4,096-load dependent pointer chase calibrates guest-observed
response, while three transfer sizes test read/write sensitivity. Sentinel
round trips and exact operation, byte, and delay-event accounting distinguish
the path from the earlier zero-traffic and local-RAM cases.

\begin{figure*}[t]
  \centering
  \includegraphics[width=\textwidth]{img/slugarch-type2-cxlmem.pdf}
  \caption{Layout-only Figure 2 mockup with no numeric data. The final panels
  will show dependent-load calibration, read/write transfer sensitivity,
  same-boot SlugArch overhead at 196 request records, and request-record-count
  scaling at 400 ns. The final caption will state n=5 independent guest boots,
  median with minimum and maximum, and one untimed warmup pass.}
  \label{fig:slugarch-type2-cxlmem}
\end{figure*}

\subsection{Paired SlugArch evaluation}

The common-path baseline retains the interpreter, raw request and response
staging through CXL.mem, readback, and equality checking. Validation, delta,
and full add their respective metadata and validation work. Each overhead is
computed within one boot before the five paired ratios are summarized. The
request-count experiment repeats one deterministic 49-request GEMM trace and
therefore measures scaling with bytes and request records, not workload
diversity.

\subsection{Fail-stop checks and scope}

The legacy offline mutation matrix remains distinct from the new labeled
post-transport guest-software payload flip. A final paper result requires all
20 post-transport streams to be rejected. CXL.cache remains unmeasured, and
the experiment does not establish physical latency or bandwidth, device-side
SlugArch execution, DMA, ATS, migration, switch ordering, recovery, or FPGA
cost.
"""


def run(args: list[str], cwd: Path) -> str:
    return subprocess.run(
        args,
        cwd=cwd,
        check=True,
        text=True,
        capture_output=True,
    ).stdout


def main_text_page(aux_path: Path) -> int:
    aux = aux_path.read_text(encoding="utf-8", errors="replace")
    match = re.search(
        r"\\newlabel\{sec:maintext-end\}\{\{[^}]*\}\{(\d+)\}",
        aux,
    )
    if match is None:
        raise RuntimeError("layout build has no main-text end label")
    return int(match.group(1))


def main() -> int:
    if DEST.exists():
        shutil.rmtree(DEST)
    shutil.copytree(
        ROOT,
        COPY,
        ignore=shutil.ignore_patterns(
            ".git",
            "main.aux",
            "main.bbl",
            "main.blg",
            "main.fdb_latexmk",
            "main.fls",
            "main.log",
            "main.out",
            "main.pdf",
        ),
    )
    (COPY / "eval.tex").write_text(MOCK_EVALUATION.lstrip(), encoding="utf-8")
    run(
        [
            "python3",
            "scripts/plot_slugarch_type2_cxlmem.py",
            "--mockup",
            "--output",
            "img/slugarch-type2-cxlmem.pdf",
        ],
        COPY,
    )
    run(["latexmk", "-C", "main.tex"], COPY)
    run(
        [
            "latexmk",
            "-pdf",
            "-interaction=nonstopmode",
            "-halt-on-error",
            "main.tex",
        ],
        COPY,
    )
    end_page = main_text_page(COPY / "main.aux")
    if end_page > 11:
        raise RuntimeError(f"layout mockup main text ends on page {end_page}")
    extracted = run(
        ["pdftotext", "-layout", "main.pdf", "-"],
        COPY,
    )
    if "LAYOUT ONLY" not in extracted.upper():
        raise RuntimeError("Figure 2 mockup is not visibly labeled")
    for figure in (
        "img/slugarch-results.pdf",
        "img/slugarch-type2-cxlmem.pdf",
    ):
        info = run(["pdfinfo", figure], COPY)
        if not re.search(r"Page size:\s+511\.2 x 216 pts", info):
            raise RuntimeError(f"wrong mockup figure size: {figure}")
    print(f"layout mockup: PASS; main text ends on page {end_page}")
    print(COPY / "main.pdf")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 3: Run the mockup gate**

Run:

```bash
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 scripts/build_slugarch_layout_mockup.py
```

Expected:

```text
layout mockup: PASS; main text ends on page 11
/tmp/slugarch-paper-layout/paper/main.pdf
```

An earlier end page also passes. A page later than 11 is a hard pre-campaign
failure.

- [ ] **Step 4: Inspect the layout mockup**

Run:

```bash
pdftocairo -png -r 160 \
  /tmp/slugarch-paper-layout/paper/main.pdf \
  /tmp/slugarch-layout-page
pdftocairo -png -singlefile -r 240 \
  /tmp/slugarch-paper-layout/paper/img/slugarch-type2-cxlmem.pdf \
  /tmp/slugarch-type2-layout
```

Inspect every `/tmp/slugarch-layout-page-*.png` and
`/tmp/slugarch-type2-layout.png`. Require:

- both figures fit without clipping;
- Figure 2 is visibly labeled as data-free;
- both captions occupy at most 90 words;
- no printed figure text is smaller than 7 points;
- evaluation occupies no more than its three-page allocation;
- there is no float-only page or empty trailing column.

If the automated or visual gate fails, shorten only duplicated prose or caption
wording, rebuild, and rerun this task. Do not launch the campaign while the
mockup fails, and do not change the experiment matrix, IEEE geometry, or
figure-text size.

- [ ] **Step 5: Commit only the mockup builder**

Run:

```bash
git add scripts/build_slugarch_layout_mockup.py
git diff --cached --check
git commit -m "test: gate the two-figure paper layout"
```

Expected: the script is committed; `/tmp/slugarch-paper-layout` and its
data-free PDFs remain outside Git.

---

## Task 8: Enforce the Complete-Campaign Gate and Render Figure 2

**Files:**
- Read only:
  `/tmp/slugarch-type2-cxlmem-paper-export/slugarch-type2-cxlmem.json`
- Read only:
  `/tmp/slugarch-type2-cxlmem-paper-export/export-checksums.sha256`
- Read only:
  `/tmp/slugarch-type2-cxlmem-paper-export/export-validation.json`
- Create: `data/slugarch-type2-cxlmem.json`
- Create: `img/slugarch-type2-cxlmem.pdf`

- [ ] **Step 1: Demonstrate that the full Figure 2 test fails before import**

Run:

```bash
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 -m unittest \
  scripts.test_plot_slugarch_type2_cxlmem.Type2FigureTests.test_complete_snapshot_and_render \
  -v
```

Expected:

```text
FAIL: complete-campaign paper snapshot is required
```

This failure is intentional. Do not create a substitute JSON.

- [ ] **Step 2: Require all three experiment handoff files**

Run:

```bash
test -f /tmp/slugarch-type2-cxlmem-paper-export/slugarch-type2-cxlmem.json
test -f /tmp/slugarch-type2-cxlmem-paper-export/export-checksums.sha256
test -f /tmp/slugarch-type2-cxlmem-paper-export/export-validation.json
```

Expected: all commands return zero. If any file is missing, stop this plan and
finish the Type-2 campaign implementation; Figure 2 remains absent.

- [ ] **Step 3: Verify the export seal and terminal validation record**

Run:

```bash
cd /tmp/slugarch-type2-cxlmem-paper-export
sha256sum -c export-checksums.sha256
jq -e '
  .status == "pass" and
  .campaign_complete == true and
  .campaign_checksum_tree_valid == true and
  .registry_hash_chain_valid == true and
  .selected_by_lowest_complete_ordinal == true and
  .committed_slots == 20 and
  .committed_boots_per_latency == 5 and
  .corruption_rejections == 20 and
  (.failed_panel_gates | length) == 0
' export-validation.json
```

Expected: every checksum prints `OK`, and `jq` prints `true`.

Any `.inprogress`, `.failed`, unregistered, superseding same-version, partial,
or checksum-invalid campaign fails here. Do not bypass this command based on
visually plausible numbers.

- [ ] **Step 4: Validate the exported snapshot before copying**

Run:

```bash
cd /tmp/slugarch-paper-integration
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 scripts/plot_slugarch_type2_cxlmem.py \
  --check-only \
  --data /tmp/slugarch-type2-cxlmem-paper-export/slugarch-type2-cxlmem.json
```

Expected:

```text
Type-2 paper snapshot: PASS
```

This validates all 20 slots, 60 transfer rows, 320 SlugArch rows, paired
same-boot ratios, 20 corruption rejections, and the predeclared calibration
and transfer monotonicity/separation rules.

- [ ] **Step 5: Copy the normalized snapshot byte-for-byte**

Run:

```bash
mkdir -p data
cp \
  /tmp/slugarch-type2-cxlmem-paper-export/slugarch-type2-cxlmem.json \
  data/slugarch-type2-cxlmem.json
cmp \
  /tmp/slugarch-type2-cxlmem-paper-export/slugarch-type2-cxlmem.json \
  data/slugarch-type2-cxlmem.json
sha256sum \
  /tmp/slugarch-type2-cxlmem-paper-export/slugarch-type2-cxlmem.json \
  data/slugarch-type2-cxlmem.json
```

Expected: `cmp` returns zero and both SHA-256 values are identical.

- [ ] **Step 6: Run the complete Figure 2 tests**

Run:

```bash
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 -m unittest scripts/test_plot_slugarch_type2_cxlmem.py -v
```

Expected:

```text
test_complete_snapshot_and_render ... ok
test_mockup_is_deterministic_and_correct_size ... ok
```

- [ ] **Step 7: Render twice and enforce a stable Figure 2 hash**

Run:

```bash
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 scripts/plot_slugarch_type2_cxlmem.py
sha256sum img/slugarch-type2-cxlmem.pdf \
  > /tmp/slugarch-type2-figure.sha256
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 scripts/plot_slugarch_type2_cxlmem.py
sha256sum -c /tmp/slugarch-type2-figure.sha256
pdfinfo img/slugarch-type2-cxlmem.pdf |
  rg '^(Pages|Page size):'
```

Expected:

```text
img/slugarch-type2-cxlmem.pdf: OK
Pages:           1
Page size:       511.2 x 216 pts
```

- [ ] **Step 8: Inspect Figure 2 directly**

Run:

```bash
pdftocairo -png -singlefile -r 240 \
  img/slugarch-type2-cxlmem.pdf /tmp/slugarch-type2-cxlmem
```

Inspect `/tmp/slugarch-type2-cxlmem.png`. Require:

- five boot points plus median/min-max in panel (a);
- separate read and write facets in panel (b);
- same-boot ratios and a 1.0 reference in panel (c);
- all four modes and record counts at 400 ns in panel (d);
- simulator configuration and guest-observed units, not link-level labels;
- no clipping, preview watermark, interpolated series, or sub-7-point text;
- legibility in grayscale.

- [ ] **Step 9: Commit only the validated snapshot and rendered figure**

Run:

```bash
git add \
  data/slugarch-type2-cxlmem.json \
  img/slugarch-type2-cxlmem.pdf
git diff --cached --check
git commit -m "data: add validated Type-2 CXL.mem paper results"
```

Expected: exactly two paths in the commit. The raw campaign remains in the
SlugArch artifact tree and is referenced by hashes and relative paths from the
snapshot; it is not copied into the paper.

---

## Task 9: Replace the Evaluation with the Two-Figure Evidence Narrative

**Files:**
- Modify: `eval.tex:1-420`
- Modify: `main.tex:121-138`
- Modify: `intro.tex:25-31`
- Modify: `related.tex:35-44`

- [ ] **Step 1: Run the final contract and record its evaluation failure**

Run:

```bash
latexmk -pdf -interaction=nonstopmode -halt-on-error main.tex
python3 scripts/check_paper_contract.py --stage final
```

Expected: FAIL because the old four-table evaluation has not yet been replaced.

- [ ] **Step 2: Replace `eval.tex` with the final evidence-first text**

Replace `eval.tex` with:

```tex
\section{Evaluation}
\label{sec:evaluation}

We evaluate two distinct artifact boundaries. The July pass checks legacy BAR2
request integration and software-only record behavior. The new campaign checks
a one-target QEMU Type-2 CXL.mem path backed by CXLMemSim, then measures
SlugArch modes on that same simulator substrate. The normalized data, source
hashes, campaign seal, and limitations are shipped with the plotting scripts;
no value is copied from the reference manuscript.

\subsection{Setup and artifact policy}

The new path uses QEMU TCG with two vCPUs and 2~GiB of guest DRAM. One Type-2
endpoint exposes a zero-based 256~MiB memory range, while the benchmark uses
the non-bypass DPA interval from 80 to 112~MiB. The guest benchmark is pinned
away from the boot vCPU; QEMU and CXLMemSim use fixed, nonoverlapping host CPU
sets. A two-way sentinel exchange proves that devdax accesses reach the
server-authoritative store. Matching guest, QEMU, and server operation IDs,
byte totals, checksums, and delay events exclude the prior zero-traffic and
local-RAM-only paths.

We use configured delays of 80, 400, 2,000, and 10,000~ns. Each point has
n=5 independent guest boots. Every committed boot performs one untimed warmup
pass followed by one timed pass; within-boot loads, request records, and RPCs
are not treated as independent samples. We report each cell as the median with
minimum and maximum. The settings are inputs to the simulator delay model, not
measurements of a physical link.

An anonymous reference manuscript motivated only the configured 80-ns,
400-ns, 2-\(\mu\)s, and 10-\(\mu\)s grid and our five-repeat
median/minimum/maximum convention~\cite{cxl-sync-costs-reference-2026};
we do not import its platform claims or performance values.

\subsection{Legacy BAR2 and software evidence}

For each legacy request, the helper wrote the 64-byte record to BAR2, read it
back, and issued a NOP. The guest software generated the corresponding
response stream for host validation. No response FLIT traverses BAR2. All five
back-to-back runs in one TCG guest boot passed, covering 245 request
write/readbacks with no readback, command, tag, dispatch, or validation error.
Figure~\ref{fig:slugarch-results}(a) reports mixed helper-loop time, not
cross-boot repeatability, end-to-end overhead, or link latency.

\begin{figure*}[t]
  \centering
  \includegraphics[width=\textwidth]{img/slugarch-results.pdf}
  \caption{Existing SlugArch evidence. (a) Five helper runs in one TCG guest
  boot; BAR2 carried request write/readback and NOP while guest software
  generated responses. (b) Software log bytes. (c) Means of 200 validator
  repetitions; dispersion was not retained. (d) Seven offline response-stream
  mutations, all rejected.}
  \label{fig:slugarch-results}
\end{figure*}

Figure~\ref{fig:slugarch-results}(b) separates captured payload from complete
serialized-log size for validation, delta, and full policies. Panel (c)
reports arithmetic means because the earlier artifact retained no per-repeat
samples. Panel (d) maps seven offline response-stream mutations to their
rejection mechanisms; it does not represent live device injection or recovery.

\subsection{CXL.mem calibration and transfer sensitivity}

The calibration follows a seeded permutation of 4,096 cache lines in 256~KiB,
with one dependent 64-bit load per line. Every boot declares exactly 4,096
reads and 32,768 read bytes, matched by QEMU and server counters. Transfer
conditions write, fence, read back, and checksum 4~KiB, 64~KiB, and 1~MiB.
Every size and delay has exact guest/QEMU/server byte agreement and zero
local-shadow, local-cache, bulk-overlay, or coherent-pool completions.

Figure~\ref{fig:slugarch-type2-cxlmem}(a) reports guest-observed nanoseconds
per dependent load; panel (b) reports guest-effective transfer throughput.
All predeclared median ordering and 10-microsecond-versus-80-nanosecond
separation gates pass. These values characterize the synchronous
QEMU-to-CXLMemSim model plus its applied delay; they are not relabeled as
hardware latency or bandwidth.

\begin{figure*}[t]
  \centering
  \includegraphics[width=\textwidth]{img/slugarch-type2-cxlmem.pdf}
  \caption{Type-2 CXL.mem calibration and SlugArch evaluation. Results use
  n=5 independent guest boots and show median with minimum and maximum after
  one untimed warmup pass. (a) Dependent-load calibration. (b) Guest-effective
  transfer sensitivity. (c) Same-boot overhead at 196 request records.
  (d) Scaling at 400 ns using deterministic repetitions of one 49-request
  trace.}
  \label{fig:slugarch-type2-cxlmem}
\end{figure*}

\subsection{Paired SlugArch overhead and scaling}

The common-path baseline retains the guest interpreter, raw 64-byte request
and response staging through CXL.mem, readback, and bytewise equality checking;
it omits SlugArch metadata and validation. Validation, delta, and full use the
same interpreter and transport while adding their policy-specific encoding and
checking. Request and guest-software response records are written to and read
back from the mapped CXL.mem range before the host's independent validation.
The simulated device transports memory bytes and does not execute SlugArch.

Figure~\ref{fig:slugarch-type2-cxlmem}(c) shows each mode divided by the same
boot's baseline at the predeclared 196-request-record size; ratios are never
formed from separately aggregated medians. Panel (d) shows all modes at 49,
196, 784, and 3,136 request records at the predeclared 400-ns setting. The
paper snapshot also retains equal response-record counts and twice-as-large
boundary-record counts. Larger cases are deterministic copies of one GEMM
trace with distinct tags, so they establish request-record- and byte-count
sensitivity rather than workload diversity.

\subsection{Fail-stop validation}

After each timed pass, the guest produced a valid full-mode stream, staged and
read it back through CXL.mem, verified the transport checksum, and then flipped
one response-payload bit in guest DRAM. The normal validator rejected all 20
post-transport guest-software payload flips as decoded-result mismatches. This
is software fail-stop evidence after successful transport, not transport-error
detection, device fault injection, or recovery.

\subsection{Measured boundary}

The campaign establishes synchronous reads and writes through one mapped QEMU
Type-2 CXL.mem region to a server-authoritative CXLMemSim backing store, plus
paired SlugArch simulator costs on that path. CXL.cache remains unmeasured.
The experiment also excludes BI/snoop validation, DMA, ATS/PASID, migration,
peer-to-peer traffic, accelerator execution, switch or multihost ordering,
hardware record generation, hardware replay or compression, recovery, and
FPGA timing, power, area, or resource cost.
```

- [ ] **Step 3: Update the abstract to the demonstrated final boundary**

Replace the final two abstract sentences in `main.tex` with:

```tex
We define the boundary property and a fail-stop operational model, then
evaluate the implemented boundary in QEMU/CXLMemSim. Existing artifacts cover
legacy Type-2 BAR2 request integration and software record policies; a
complete 20-boot campaign calibrates a server-authoritative Type-2 CXL.mem path
and reports paired validation, delta, and full-mode costs. These simulator
results do not establish CXL.cache behavior or device-side replay.
```

- [ ] **Step 4: Remove conditional wording after the campaign has passed**

In `intro.tex`, replace:

```tex
  \item a scoped artifact showing guest-visible Type-2 BAR2 integration,
  software record tradeoffs, and, subject to the complete-campaign gate,
  calibrated Type-2 CXL.mem behavior.
```

with:

```tex
  \item scoped artifacts showing guest-visible Type-2 BAR2 integration,
  software record tradeoffs, and calibrated server-authoritative Type-2
  CXL.mem behavior across 20 independent guest boots.
```

In `related.tex`, replace:

```tex
the legacy BAR2 boundary and, after the complete campaign gate passes, one
server-authoritative QEMU/CXLMemSim Type-2 CXL.mem path.
```

with:

```tex
the legacy BAR2 boundary and one complete, server-authoritative
QEMU/CXLMemSim Type-2 CXL.mem campaign.
```

- [ ] **Step 5: Build and run the final evidence checks**

Run:

```bash
latexmk -pdf -interaction=nonstopmode -halt-on-error main.tex
python3 scripts/check_paper_contract.py --stage final
rg -n -i '\\cfr\b|\bCFR\b|CXL Fabric Replay' -- *.tex main.aux main.bbl
rg -n \
  'Undefined control sequence|LaTeX Error|undefined references|undefined citations|Citation .* undefined|Reference .* undefined' \
  main.log
```

Expected:

```text
paper contract (final): PASS
```

All searches must be empty. If only the page gate fails, continue to Task 10;
all data, naming, figure, and claim checks must already pass.

- [ ] **Step 6: Commit the final evidence narrative**

Run:

```bash
git add main.tex intro.tex eval.tex related.tex
git diff --cached --check
git commit -m "paper: integrate validated Type-2 CXL.mem evaluation"
```

Expected: exactly four TeX files in the commit.

---

## Task 10: Enforce the 11-Page Gate and Perform Full Visual QA

**Files:**
- Modify only if the page gate requires it: `main.tex`, `design.tex`,
  `semantics.tex`, `eval.tex`
- Regenerate: `main.aux`, `main.bbl`, `main.pdf`

- [ ] **Step 1: Run the clean final build and all automated checks**

Run:

```bash
latexmk -C main.tex
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 -m unittest \
  scripts/test_plot_slugarch_results.py \
  scripts/test_plot_slugarch_type2_cxlmem.py \
  -v
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 scripts/plot_slugarch_results.py
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 scripts/plot_slugarch_type2_cxlmem.py
latexmk -pdf -interaction=nonstopmode -halt-on-error main.tex
python3 scripts/check_paper_contract.py --stage final
```

Expected: all four plot tests pass and the paper contract prints:

```text
paper contract (final): PASS
```

- [ ] **Step 2: Apply only the ranked prose cuts if the page gate fails**

Rebuild and rerun the final contract after each cut. Stop immediately when the
contract passes.

Cut 1 — replace the body of `design.tex` subsection
`Cross-protocol ordering` with:

```tex
SlugArch orders covered events across CXL.cache, CXL.mem, CXL.io, and
device-specific queues through explicit dependency records rather than a
fabric-wide cycle count. A bridge record relates submission, memory
visibility, fence, and completion when the installed boundary observes them.
An unobserved protocol transition blocks the corresponding claim.
```

Cut 2 — replace the body of `design.tex` subsection `Epochs and stalls` with:

```tex
Epochs bound recording, sealing, and replay progress. Replay may stall an early
event until its covered predecessors arrive, but it rejects a dependency cycle,
missing predecessor, incompatible policy change, or exhausted progress bound.
```

Cut 3 — replace the first two paragraphs of `eval.tex` subsection
`Setup and artifact policy` with:

```tex
The new TCG path uses two vCPUs, 2~GiB of guest DRAM, one zero-based 256~MiB
Type-2 range, and a non-bypass 80--112~MiB DPA work window. A two-way sentinel
and matching guest/QEMU/server IDs, bytes, checksums, and delay events prove
that devdax reaches the server-authoritative store. Four configured delays each
use n=5 independent guest boots, one untimed warmup pass, and one timed pass;
cells report median with minimum and maximum. Settings characterize the
simulator model, not a physical link.
```

Cut 4 — replace the Figure 1 caption with:

```tex
\caption{Existing SlugArch evidence: (a) five BAR2 helper runs in one TCG
guest boot; (b) software log bytes; (c) means of 200 validator repetitions
without retained dispersion; and (d) seven rejected offline mutations. BAR2
carried request write/readback and NOP; guest software generated responses.}
```

Cut 5 — replace the Figure 2 caption with:

```tex
\caption{Type-2 CXL.mem results for n=5 independent guest boots, reported as
median with minimum and maximum after one untimed warmup pass: (a)
dependent-load calibration; (b) transfer sensitivity; (c) paired overhead at
196 request records; and (d) repeated-trace request scaling at 400 ns.}
```

Do not change the IEEE class, margins, font size, line spacing, figure
dimensions, or 7-point minimum figure text.

- [ ] **Step 3: Verify stable figure hashes after the final source state**

Run:

```bash
sha256sum \
  img/slugarch-results.pdf \
  img/slugarch-type2-cxlmem.pdf \
  > /tmp/slugarch-final-figures.sha256
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 scripts/plot_slugarch_results.py
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 scripts/plot_slugarch_type2_cxlmem.py
sha256sum -c /tmp/slugarch-final-figures.sha256
```

Expected:

```text
img/slugarch-results.pdf: OK
img/slugarch-type2-cxlmem.pdf: OK
```

- [ ] **Step 4: Check build logs, names, captions, and page labels**

Run:

```bash
python3 scripts/check_paper_contract.py --stage final
rg -n \
  'Undefined control sequence|LaTeX Error|undefined references|undefined citations|Citation .* undefined|Reference .* undefined|Overfull \\hbox' \
  main.log
rg -n -i \
  '\\cfr\b|\bCFR\b|CXL Fabric Replay|\\citep|ACM-Reference-Format' \
  -- *.tex main.aux main.bbl
pdfinfo main.pdf | rg '^(Title|Pages|Page size):'
rg -n 'sec:maintext-end' main.aux
pdftotext -layout main.pdf /tmp/slugarch-final-layout.txt
rg -n -i '\bCFR\b|CXL Fabric Replay' /tmp/slugarch-final-layout.txt
```

Expected:

- contract PASS;
- warning/name searches empty;
- PDF page size `612 x 792 pts (letter)`;
- `sec:maintext-end` names a page no greater than 11.

Total PDF pages may exceed 11 because references are exempt.

- [ ] **Step 5: Inspect every paper page and both figures**

Run:

```bash
pdftocairo -png -r 160 main.pdf /tmp/slugarch-final-page
pdftocairo -png -singlefile -r 240 \
  img/slugarch-results.pdf /tmp/slugarch-results-final
pdftocairo -png -singlefile -r 240 \
  img/slugarch-type2-cxlmem.pdf /tmp/slugarch-type2-final
```

Inspect every generated PNG. Require:

- the conclusion finishes on or before page 11;
- no clipped figure, legend, caption, equation, or table remnant;
- no float-only page, orphan heading, or excessive blank column;
- both figures are legible at printed two-column width and in grayscale;
- Figure 1 preserves one-boot/offline qualifiers;
- Figure 2 contains no mockup label and no partial panel;
- references begin after the labeled main-text endpoint.

- [ ] **Step 6: Invoke independent review and completion verification**

Use `superpowers:requesting-code-review` to review:

- both approved design specifications;
- campaign eligibility and data provenance;
- BAR2 versus CXL.mem wording;
- paired-ratio computation;
- Figure 2 all-or-nothing gates;
- blocked CXL.cache/hardware claims;
- the 11-page PDF.

Address confirmed findings, rerun Steps 1, 3, 4, and 5, then invoke
`superpowers:verification-before-completion`.

- [ ] **Step 7: Commit the verified paper and generated deliverables**

Run:

```bash
git add \
  main.tex intro.tex background.tex design.tex semantics.tex eval.tex related.tex \
  main.aux main.bbl main.pdf \
  data/slugarch-results-20260704.json \
  data/slugarch-type2-cxlmem.json \
  scripts/check_paper_contract.py \
  scripts/plot_slugarch_results.py \
  scripts/test_plot_slugarch_results.py \
  scripts/plot_slugarch_type2_cxlmem.py \
  scripts/test_plot_slugarch_type2_cxlmem.py \
  scripts/build_slugarch_layout_mockup.py \
  img/slugarch-results.pdf \
  img/slugarch-type2-cxlmem.pdf
git diff --cached --check
git commit -m "paper: finalize 11-page SlugArch evaluation"
```

Expected: Git stages only approved paper sources, generated bibliography state,
the two reviewed snapshots, paper tooling, two figures, and `main.pdf`.
Already committed unchanged paths are harmlessly omitted from this final commit.

---

## Task 11: Review the Isolated Diff, Synchronize the Allowlist, and Reverify

**Files synchronized to `/root/Concordia/64fa450c44d0cdf46c7c3a7d`:**

```text
main.tex
intro.tex
background.tex
design.tex
semantics.tex
eval.tex
related.tex
cite.bib
main.aux
main.bbl
main.pdf
data/slugarch-results-20260704.json
data/slugarch-type2-cxlmem.json
scripts/check_paper_contract.py
scripts/plot_slugarch_results.py
scripts/test_plot_slugarch_results.py
scripts/plot_slugarch_type2_cxlmem.py
scripts/test_plot_slugarch_type2_cxlmem.py
scripts/build_slugarch_layout_mockup.py
img/slugarch-results.pdf
img/slugarch-type2-cxlmem.pdf
```

- [ ] **Step 1: Review the complete branch diff**

Run:

```bash
git status --short --branch
git log --oneline --decorate --reverse master..HEAD
git diff master...HEAD --stat
git diff master...HEAD --check
git diff master...HEAD -- \
  main.tex intro.tex background.tex design.tex semantics.tex eval.tex related.tex
```

Expected: no unrelated source, old plan, raw campaign artifact, mockup PDF, or
reference-manuscript PDF appears in the diff.

- [ ] **Step 2: Recheck the destination before synchronization**

Run:

```bash
git -C /root/Concordia/64fa450c44d0cdf46c7c3a7d rev-parse HEAD
git -C /root/Concordia/64fa450c44d0cdf46c7c3a7d status --short --branch
git -C /root/Concordia/64fa450c44d0cdf46c7c3a7d diff --quiet -- \
  main.tex intro.tex background.tex design.tex semantics.tex eval.tex related.tex cite.bib main.bbl
```

Expected HEAD:

```text
99698736f7582c7112f52679544e2478525d9deb
```

The quiet diff command must return zero. The pre-existing dirty `main.aux` and
untracked build outputs may remain; they are replaced only because the approved
paper workflow explicitly regenerates them. If a human changed any TeX source,
`cite.bib`, or `main.bbl`, stop and reconcile instead of overwriting it.

- [ ] **Step 3: Synchronize only the approved allowlist**

After the sandbox approval prompt, run:

```bash
mkdir -p \
  /root/Concordia/64fa450c44d0cdf46c7c3a7d/data \
  /root/Concordia/64fa450c44d0cdf46c7c3a7d/scripts \
  /root/Concordia/64fa450c44d0cdf46c7c3a7d/img
cp main.tex intro.tex background.tex design.tex semantics.tex eval.tex related.tex \
  cite.bib main.aux main.bbl main.pdf \
  /root/Concordia/64fa450c44d0cdf46c7c3a7d/
cp data/slugarch-results-20260704.json \
  data/slugarch-type2-cxlmem.json \
  /root/Concordia/64fa450c44d0cdf46c7c3a7d/data/
cp scripts/check_paper_contract.py \
  scripts/plot_slugarch_results.py \
  scripts/test_plot_slugarch_results.py \
  scripts/plot_slugarch_type2_cxlmem.py \
  scripts/test_plot_slugarch_type2_cxlmem.py \
  scripts/build_slugarch_layout_mockup.py \
  /root/Concordia/64fa450c44d0cdf46c7c3a7d/scripts/
cp img/slugarch-results.pdf \
  img/slugarch-type2-cxlmem.pdf \
  /root/Concordia/64fa450c44d0cdf46c7c3a7d/img/
```

- [ ] **Step 4: Verify source and destination hashes**

Run from `/tmp/slugarch-paper-integration`:

```bash
for path in \
  main.tex intro.tex background.tex design.tex semantics.tex eval.tex related.tex \
  cite.bib main.aux main.bbl main.pdf \
  data/slugarch-results-20260704.json \
  data/slugarch-type2-cxlmem.json \
  scripts/check_paper_contract.py \
  scripts/plot_slugarch_results.py \
  scripts/test_plot_slugarch_results.py \
  scripts/plot_slugarch_type2_cxlmem.py \
  scripts/test_plot_slugarch_type2_cxlmem.py \
  scripts/build_slugarch_layout_mockup.py \
  img/slugarch-results.pdf \
  img/slugarch-type2-cxlmem.pdf
do
  test "$(sha256sum "$path" | cut -d' ' -f1)" = \
       "$(sha256sum "/root/Concordia/64fa450c44d0cdf46c7c3a7d/$path" | cut -d' ' -f1)"
done
```

Expected: every comparison returns zero.

- [ ] **Step 5: Rebuild and verify in the destination**

Run:

```bash
cd /root/Concordia/64fa450c44d0cdf46c7c3a7d
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 -m unittest \
  scripts/test_plot_slugarch_results.py \
  scripts/test_plot_slugarch_type2_cxlmem.py \
  -v
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 scripts/plot_slugarch_results.py
MPLCONFIGDIR=/tmp/slugarch-mpl \
  python3 scripts/plot_slugarch_type2_cxlmem.py
latexmk -C main.tex
latexmk -pdf -interaction=nonstopmode -halt-on-error main.tex
python3 scripts/check_paper_contract.py --stage final
pdfinfo main.pdf | rg '^(Title|Pages|Page size):'
git status --short --branch
```

Expected:

- all plot tests pass;
- final paper contract passes;
- main text ends by page 11;
- destination status contains only the approved changed paths plus the
  pre-existing generated files that are not part of the allowlist.

- [ ] **Step 6: Report the exact completed boundary**

Report:

- source and destination commit IDs;
- final `sec:maintext-end` page and total PDF pages;
- SHA-256 values for both JSON snapshots and both figure PDFs;
- 7.1-by-3.0-inch dimensions and four-panel composition of each figure;
- complete-campaign ID, registry ordinal, experiment-version hash, and campaign
  checksum hash from the Type-2 snapshot;
- CFR-to-SlugArch source/PDF search results;
- test, build, contract, and visual-QA outcomes;
- legacy BAR2 request-only boundary;
- new server-authoritative CXL.mem simulator boundary;
- continued exclusion of CXL.cache and all unmeasured hardware claims.

Do not describe the paper as complete if the destination rebuild differs from
the isolated clone, any panel gate is false, the campaign is incomplete, or the
main-text label exceeds page 11.

---

## Completion Checklist

- [ ] Isolated clone remained the only implementation workspace.
- [ ] Source checkout and SlugArch raw artifacts were not cleaned or staged.
- [ ] CFR and CXL Fabric Replay are absent from source, auxiliary files, PDF
  text, bookmarks, and metadata.
- [ ] IEEE build defaults, theorem environments, citations, and bibliography
  style are valid.
- [ ] Figure 1 comes only from the three reviewed July sources and preserves
  one-boot/offline qualifiers.
- [ ] Figure 2 comes only from the checksum-valid, eligible, complete 20-boot
  campaign.
- [ ] Both figures are deterministic 7.1-by-3.0-inch vector PDFs with at least
  7-point text and captions no longer than 90 words.
- [ ] Paired overhead uses each mode and baseline from the same boot.
- [ ] The evaluation uses no result tables and contains both full-width
  figures.
- [ ] The conclusion finishes on or before page 11 without changing IEEE
  geometry.
- [ ] CXL.cache, hardware execution, and other excluded claims remain explicit.
- [ ] Only the reviewed synchronization allowlist was copied to the paper
  checkout.
