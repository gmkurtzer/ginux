# Gate 7: SMP Foundations

## Goal

Run Ginux correctly on fixed 1-, 2-, and 4-vCPU x86-64 systems.

## Approved scope

- ACPI/MADT processor discovery and application-processor startup.
- Per-CPU GDT, TSS, IST, stacks, current task, and local APIC timers.
- IPIs, cross-CPU wakeups, scheduler operation, and TLB shootdowns.
- Domain-aware placement, accounting, limits, priorities, and QoS.

## Non-goals

- CPU hotplug.
- Architectures other than x86-64.

## Completed work

None. Depends on Gate 6.

## Remaining work

- Specify AP startup and per-CPU lifecycle.
- Implement cross-CPU mechanisms and scheduler integration.
- Add deterministic and stress coverage at each CPU count.

## Acceptance criteria

- Ginux boots reliably with one, two, and four vCPUs.
- Linux userspace reports and uses the configured CPU count.
- Processes and threads execute concurrently on different CPUs.
- Cross-CPU signals, futexes, pipes, sockets, process exit, and mapping changes
  work correctly.
- Domain CPU limits and priorities are enforced.
- Repeated stress produces no deadlock, corruption, stale translation, or
  kernel panic.

## Evidence

CPU-topology transcripts, concurrency proofs/tests, scheduler and shootdown
traces, multi-domain policy results, and stress runs.

## Decisions and limitations

Hotplug remains deferred unless separately approved after this gate.

## Delivery strategy

Planned commits: discovery/AP startup, per-CPU state, IPIs/shootdowns,
scheduler concurrency, domain policy, and integration evidence.

## Review policy and outcome

Checkpoint required. Stop for SMP function and scheduling-policy review.
Outcome: pending.
