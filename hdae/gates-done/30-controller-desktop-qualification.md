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

- [x] Added an integrated host and QEMU desktop qualification target covering
  every display, windowing, controller, bridge, Wayland, presentation, and
  recovery contract at two and four virtual CPUs.
- [x] Added release-mode startup, input-to-frame, frame, recovery, and bounded
  memory measurements with published budgets and raw output retention.
- [x] Ran the clean static, ABI, SMP, controller, storage, Rocky shell, Rocky
  systemd, and concurrent-domain regressions without weakening behavior.
- [x] Reconciled a stale ABI report expectation with the already accepted
  compatibility-ledger semantics and reran the complete 398-row report.
- [x] Documented reproduction, measurements, architecture, recovery, and the
  explicit unsupported desktop surface.
- [x] Audited process cleanup, document links, and forward/reverse/coverage/
  orphan traceability for Gates 19–30.

## Remaining work

- [x] None.

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

- `make clean && make check` -> passed from a clean tree, including
  architecture policy, all 375 syscall classifications, formatting, clippy,
  native C ABI, and crate tests.
- `make test-controller-desktop-all` -> passed all seven desktop component
  suites, the complete QEMU verification boot, and one recovery boot at each of
  two and four vCPUs.
- `make test-controller-desktop-performance` -> passed at 800x600 over 500
  samples: 1 us startup, 421 us input-to-frame p95 with a background domain,
  277 us steady-frame p95, below-resolution recovery p95, and a 1,950,000-byte
  modeled bounded working set.
- `make test-smp` -> passed 1-, 2-, and 4-vCPU qualification plus two repeated
  2-vCPU stress boots.
- `make test-controller-console` and `make test-storage` -> passed controller
  operation and persistent recovery/reset.
- `make test-linux-abi` -> passed all 23 interfaces and 375 syscalls after the
  test/spec totals were aligned with the accepted 53 Complete, 257 Partial, 17
  Unsupported, and 48 Policy-denied ledger.
- `make test-rocky-systemd` -> passed cold, cached, and detached starts; the
  cold QEMU-start-to-multi-user result was 40,342 ms and cached was 18,517 ms,
  with Linux-init-to-console about 9.5 seconds.
- `make test-rocky-shell` and `make test-multi-domain` -> passed clean-volume
  glibc execution and concurrent Rocky lifecycle.
- QEMU process and volume-handle audit -> no QEMU process or open Rocky volume
  remained. Initial cold-pull failures were traced by packet capture to the
  execution sandbox returning ICMP network-unreachable; the authorized
  network-enabled run passed without a Ginux code change.
- `git diff --check`, documentation target/link inspection, gate-ledger review,
  and `git log origin/main..HEAD` -> no whitespace failure, broken local desktop
  link, unchecked Gate 19–30 work, or branch change outside the approved batch.

## Decisions and limitations

- Qualification is for the recorded QEMU software-rendered environment and the
  selected Wayland clients, not general hardware or desktop compatibility.
- Performance values cover the release-mode desktop state machines and software
  renderer on the recorded host. They are not whole-system CPU/RSS, GPU,
  physical-display, or end-user latency measurements.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: shared integrated PR for Gates 19–30
- Planned commits: qualification harness; performance evidence; documentation
- Actual commits: `85f599b2` (qualification harness, budgets, and docs),
  `cc138367` (ABI report consistency), and the gate-closure commit.

## Review policy and outcome

Passed and closed on 2026-10-31 under the human-confirmed delegated batch. The
integrated branch is ready for human review before publication.
