# Controller desktop qualification and documentation

## Goal

Qualify the complete native desktop and Linux-domain experience for correctness,
isolation, responsiveness, regressions, reproducibility, and documentation.

## Approved scope

- Run the complete desktop, multi-domain, SMP, failure, and existing regression
  suites from a clean build.
- Establish startup, input-to-frame, frame-time, CPU, and memory budgets and
  record measured results without benchmark-specific shortcuts.
- Document operator workflows, architecture, protocol, native application API,
  Linux Wayland limits, failure recovery, and future work.
- Perform forward/reverse/coverage/orphan reconciliation for Gates 19–30.

## Non-goals

- Production certification or claims beyond tested QEMU hardware and clients.

## Completed work

- [ ] None.

## Remaining work

- [ ] Define budgets from measured baseline, run qualification, reconcile
  documentation, and prepare the integrated PR evidence.

## Acceptance criteria

- Clean-build QEMU tests demonstrate native controller boot, native windows,
  graphical management, two isolated domains, Linux Wayland clients, windowed
  and full-screen presentation, secure attention, and recovery.
- No test leaks QEMU processes or persistent temporary resources.
- Input-to-frame and steady-state frame time remain responsive under a running
  background domain; budgets and limitations are published with measurements.
- Existing `make check`, SMP, controller, storage, Linux ABI, and Rocky domain
  suites pass without weakened expectations.
- Documentation permits another developer to reproduce, use, and extend the
  tested implementation and states all unsupported features honestly.
- Every criterion in Gates 19–30 has fresh evidence and no orphaned change.

## Verification plan

- `make clean && make check` -> full clean regression passes.
- `make test-controller-desktop-all` -> integrated desktop qualification passes.
- `make test-controller-desktop-performance` -> published budgets pass with raw
  measurements retained.
- `pgrep -f 'qemu-system.*ginux'` after tests -> no test-owned QEMU process remains.
- Documentation link and HDAE coverage audit -> no broken link or unmapped item.

## Evidence

- Not yet run.

## Decisions and limitations

- Qualification is for the recorded QEMU software-rendered environment and the
  selected Wayland clients, not general hardware or desktop compatibility.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: shared integrated PR for Gates 19–30
- Planned commits: qualification harness; performance evidence; documentation
- Actual commits: pending

## Review policy and outcome

Delegated closure under the approved batch after all criteria pass, followed by
a batch completion report and human review of the integrated pull request.
