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
- [ ] Run the same command in public GitHub CI and retain logs and input hashes.
- [ ] Demonstrate the spinner oracle rejects an unpatched baseline.
- [ ] Run make check and complete QEMU integration; record any failures exactly.
- [ ] Update PR documentation and deliver the tested commits for maintainer review.

## Verification plan

Compile the existing static PIE fixtures, construct an empty 64 MiB volume with
seed-benchmark-cache, and boot isolated copies of that volume. Test 1/2/4 CPUs;
retain CPU masks, full register sentinels, quiet competition, signal counts,
timed waits and clean exits. The cooperative control must pass. Unpatched
main must fail the no-syscall spinner case. No skipped case counts as passing.
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
EAGAIN after background handoff, preserving desktop input processing. Fresh cache construction passes on Linux. `make check` passes on the integrated
candidate on Tamm. Candidate KVM cooperative/spinner/context have passed on one
and two CPUs; the rest of KVM and the TCG matrix are running. Full TCG boot is
running separately with a separate ESP, preserving the normal acceptance image.

Diagnostic failures retained on Tamm: immediate second-domain startup raced the
single cache/install request slot; setup now waits for first userspace entry.
A late preceding prompt broke the stop oracle; status parsing now waits for
Execution and the following prompt. Single-domain controls passed KVM and TCG.
Pre-fix main c8d12653 failed the cooperative control, so it cannot establish the
spinner defect independently; the pinned pre-desktop baseline is 7fdc880e. Its
TCG cooperative control passes; no-syscall negative control is running.

The contributor run directories and process handles are retained in the ignored
local `build/scheduler-worklog.json`. Do not alter the normal EFI while a matrix
is running. Next: collect every result, finish public CI delivery and update
PR #85.

Active skill: .agents/skills/hdae-development/SKILL.md; reload the complete skill,
its method reference and project checkpoint after every context shift.
