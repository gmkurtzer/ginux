# Reproducible acceptance for detached-domain preemption

## Goal and approved scope

Address the maintainer's PR #85 review: build all targeted scheduler fixtures
from a clean checkout and run them on maintainer-accessible infrastructure.
The user requested continuation of this existing PR and supplied the acceptance
blocker. Preserve the detached-domain scope and existing sentinel checks.

## Work and acceptance criteria

- [x] Integrate current main, including #84 and the native controller desktop.
- [x] Build the payloads and a fresh offline cache using repository tools only.
- [x] Provide one command for cooperative/spinner, 1/2/4 CPU context, sibling,
  signal/timed-wait and active-worker stop/restart cases.
- [x] Run the same command in public GitHub CI and retain logs and input hashes.
- [x] Demonstrate the spinner oracle rejects an unpatched baseline.
- [x] Run make check and complete QEMU integration; record any failures exactly.
- [x] Update reproduction documentation and publish the acceptance implementation.
- [ ] Obtain maintainer acceptance after checking CI for the current PR head.

## Verification plan

Compile the existing static PIE fixtures, construct an empty 64 MiB volume with
seed-benchmark-cache, and boot isolated copies of that volume. Test 1/2/4 CPUs;
retain CPU masks, full register sentinels, quiet competition, signal counts,
timed waits and clean exits. The cooperative control must pass. Pinned pre-fix
7fdc880e must fail the no-syscall spinner case. No skipped case counts as passing.
Run make check and make test on the integrated candidate. Preserve logs and JSON
results, including compiler, accelerator, source and binary identities.

## Delivery and checkpoint

Branch: fix/background-domain-preemption. Base: main. Existing PR: #85.
Planned commits: main integration; reproducible fixture/CI delivery; evidence
and review documentation. No new product feature or merge is authorized here.
Keep this gate open until the evidence is complete and leave final acceptance
to the maintainer. The user's publication instruction covers updates to the existing PR.

## Evidence and exact next action

Main c8d12653 integrated in 713ca279. Its nonblocking controller read returns
EAGAIN after background handoff, preserving desktop input processing. Commit
844d7ea2 adds source-built fixtures and CI; 4601f0cf corrects the cross-domain
overlap clock; 38a8d1de adds full public integration; 023fc00c requires continuous
workers to remain active after every restart.

- Public corrected-clock TCG matrix: all 1/2/4 CPU cells pass in
  [run 41803195004](https://github.com/gmkurtzer/ginux/actions/runs/41803195004)
  at head 4601f0cf, including the cooperative/negative baseline controls.
- Public full deterministic `make test`: passes at 38a8d1de in
  [run 41803476665, full-integration](https://github.com/gmkurtzer/ginux/actions/runs/41803476665/job/126484674504).
  The intervening change only adds that workflow job. Static and QEMU smoke
  checks pass there as well.
- Contributor Linux `make check` and all twelve KVM cases pass. Context/sibling
  cases were rerun with the shared-clock correction. The stricter continuous
  restart guard passes on KVM and TCG. Compact reports and input identities:
  `docs/scheduler/evidence/acceptance-20261104.json`.
- Contributor Linux full `make test` fails in native-volume lifecycle on both
  the candidate and unmodified c8d12653, at different stages. These are failures,
  not evidence of a common root cause; details are in `docs/scheduler/REVIEW.md`.

Diagnostic failures and their corrections: overlapping domain setup raced the
single cache/install slot; setup now waits for first userspace entry. Late serial
prompts broke the status oracle; it now waits for Execution and its trailing
prompt. Per-domain CLOCK_MONOTONIC cannot prove overlap; the corrected fixture
uses the shared CLOCK_REALTIME epoch. The first public run's overlap result is
explicitly superseded. Pre-fix c8d12653 failed the cooperative control; the valid
paired oracle instead uses pinned pre-desktop 7fdc880e with the same fixtures.

Detailed private run locations stay in ignored `build/scheduler-worklog.json`.
Public documents must not include contributor hostnames, account names, internal
paths or infrastructure addresses. Next: inspect CI for the final publication
head, update PR #85 with its links, and leave acceptance/merge to the maintainer.

Active skill: .agents/skills/hdae-development/SKILL.md; reload the complete skill,
its method reference and project checkpoint after every context shift.
