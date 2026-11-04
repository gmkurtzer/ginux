# Kernel TCP monotonic clock

## Source

[GitHub issue #71](https://github.com/gmkurtzer/ginux/issues/71).

## Goal

Drive the kernel smoltcp runtime from calibrated elapsed time so socket API
traffic cannot accelerate protocol timers and idle wall time is not discarded.

## Approved scope

- Derive smoltcp milliseconds from the calibrated monotonic TSC.
- Preserve a monotonic, saturating clock across repeated polls.
- Poll the kernel network runtime from the existing Linux timer tick so due
  retransmit, keepalive, persist, and close timers progress without syscalls.
- Add deterministic clock arithmetic and complete QEMU regression evidence.

## Non-goals

- Selecting the half-open or established-connection timeout policies tracked by
  issues #64 and #65.
- Changing TCP backlog, close, or terminal-state semantics.

## Completed work

- [x] Confirmed the current runtime increments one millisecond per socket poll.
- [x] Confirmed the Linux execution timer already provides a serialized periodic
  kernel entry suitable for bounded network polling.

## Remaining work

- [x] Implement calibrated monotonic time and timer-driven polling.
- [x] Add deterministic regression evidence.
- [x] Run `make check` and `make test`.
- [x] Push the dedicated branch and open a pull request linked to issue #71.

## Acceptance criteria

- Repeated polls without elapsed TSC time do not advance smoltcp time.
- Elapsed wall time advances smoltcp by the corresponding milliseconds,
  monotonically and without overflow.
- The Linux timer tick polls an initialized kernel network runtime so protocol
  deadlines can progress during syscall-idle execution.
- Existing static, host, QEMU, and Linux runtime tests pass.

## Verification plan

- Deterministic kernel clock verifier -> busy-poll, elapsed-time, regression,
  unavailable-frequency fallback, and saturation cases pass.
- `make check` -> formatting, lint, architecture, unsafe inventory, and unit
  checks pass.
- `make test` -> complete deterministic QEMU and Linux runtime suite passes and
  emits the new kernel-network-clock marker.

## Evidence

- `kernel/src/linux_socket.rs::poll` currently increments `runtime.tick` once
  per call and passes it directly to smoltcp.
- The registry network service already uses calibrated TSC milliseconds, which
  establishes the repository's intended clock source.
- `make check` -> passed all formatting, lint, architecture, unsafe-inventory,
  and host-side checks.
- `make test` -> passed the complete deterministic QEMU and Linux runtime suite;
  the boot harness required `ginux: kernel network monotonic clock verified`
  and ended with `Boot test passed`.

## Decisions and limitations

- Timeout durations remain separate policy gates; this gate fixes only the time
  base and idle progression prerequisite.
- Timer-driven polling reuses the existing serialized kernel-entry lock and
  does not introduce another interrupt or worker.

## Delivery strategy

- Working branch: `hdae/issue-71-tcp-clock`
- Intended base: `main`
- Pull request: [#100](https://github.com/gmkurtzer/ginux/pull/100), dedicated
  to and closing issue #71 after merge
- Planned commits: gate and implementation with deterministic regression;
  verification evidence and delivery checkpoint
- Actual commits: `8407d4d0` (gate, implementation, regression, and initial
  evidence); `94ef74b1` (validation checkpoint); delivery record pending

## Review policy and outcome

Delegated by the authorized hourly repository-maintenance policy. Technical
passage permits pull-request publication; merge remains subject to the saved
two-pass exact-head policy.
