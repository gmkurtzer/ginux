# Gate 9: SMP Operation and 2-vCPU Test Default

## Goal

Run Ginux correctly on fixed 1-, 2-, and 4-vCPU x86-64 systems and continuously
exercise SMP by making two vCPUs the ordinary QEMU test default.

## Approved scope

- ACPI/MADT processor discovery and application-processor startup.
- Per-CPU GDT, TSS, IST, stacks, current task, and local APIC timers.
- IPIs, cross-CPU wakeups, scheduler operation, and TLB shootdowns.
- Domain-aware placement, accounting, limits, priorities, and QoS.
- QEMU test harness defaults and explicit topology matrix.

## Non-goals

- CPU hotplug.
- Architectures other than x86-64.

## Completed work

None. Depends on Gate 8.

## Remaining work

- Specify AP startup and per-CPU lifecycle.
- Implement cross-CPU mechanisms and scheduler integration.
- Add deterministic and stress coverage for one, two, and four vCPUs.
- Change ordinary QEMU tests to two vCPUs only after SMP criteria pass.

## Acceptance criteria

- Ginux boots reliably with one, two, and four vCPUs.
- Linux userspace reports and uses the configured CPU count.
- Processes and threads execute concurrently on different CPUs.
- Cross-CPU signals, futexes, pipes, sockets, process exit, and mapping changes
  work correctly.
- Domain CPU limits, placement, priorities, and accounting are enforced.
- Repeated stress produces no deadlock, corruption, stale translation, or
  kernel panic.
- Ordinary QEMU regression targets default to two vCPUs; explicit 1/2/4-vCPU
  qualification remains available and mandatory.

## Evidence

CPU-topology transcripts, concurrency tests, scheduler and shootdown traces,
multi-domain policy results, QEMU harness diffs, and repeated stress runs.

## Decisions and limitations

Hotplug remains deferred. The one-vCPU path remains supported and tested even
though two vCPUs become the ordinary development default.

## Delivery strategy

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: the shared ten-gate pull request.
- Planned commits: discovery/AP startup, per-CPU state, IPIs/shootdowns,
  scheduler concurrency, domain policy, QEMU defaults, and integration evidence.
- Actual commits: pending.

## Review policy and outcome

Delegated closure authorized after all 1/2/4-vCPU functionality, policy, stress,
and QEMU-default criteria pass. Outcome: pending.
