# Ginux AI Systems Engineering Report

This directory contains the arXiv-style technical-report draft describing the
Ginux experiment at commit `9afa61762a66a20855ea3a23e3e04221b54a5a22`.

The paper is intentionally a research stopping-point report, not a production
readiness claim. Quantitative artifact measurements in the draft were taken
from the evaluated commit. The approximately USD 8,000 API cost and the
approximately 99 percent single-agent development share are author-reported
estimates; active human hours and provider-exported token totals were not
instrumented and must not be inferred from repository history.

The simplest build uses Tectonic:

```sh
cd docs/paper
tectonic ginux-ai-systems-report.tex
```

The resulting `ginux-ai-systems-report.pdf` is ready to open in any PDF
viewer. A standard LaTeX distribution also works:

```sh
cd docs/paper
latexmk -pdf ginux-ai-systems-report.tex
```

Before publication, update the commands near the top of the TeX source,
replace the draft repository URL, record active human time if it can be
reconstructed defensibly, archive raw API billing evidence, and rerun every
reported acceptance test from a clean checkout of the evaluated commit.
