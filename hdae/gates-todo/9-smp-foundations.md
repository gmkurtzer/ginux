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

- [x] Preserve the bounded enabled-processor APIC-ID set from MADT local-APIC
  and x2APIC entries, rejecting duplicates and unsupported topology sizes.
- [x] Add an explicit `GINUX_CPUS=1|2|4` QEMU control while retaining one CPU
  as the default until SMP qualification passes.
- [x] Replace singleton GDT, TSS, IDT, privilege stack, and NMI, double-fault,
  and machine-check IST stacks with four guarded per-CPU instances; publish
  interrupt-gate changes consistently to every CPU's private IDT.
- [x] Replace the initial page-table builder's arbitrary 128-region fine-map
  cap with the complete bounded 4 GiB bootstrap-map geometry.

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

- `cargo fmt --all`, `make architecture-check`, and `make check` pass after
  preserving processor identities and adding bounded QEMU topology selection.
- `make build-verification && GINUX_CPUS=2 bash tests/qemu/boot.sh` reports
  `enabled processors: 2`, `processor APIC IDs: 0 1`, completes the complete
  deterministic kernel and Linux-runtime suite, and ends with
  `Boot test passed`. At this increment only the BSP executes; AP startup is
  the next required milestone and no SMP claim is made from discovery alone.
- The first guarded per-CPU stack build failed closed with paging error 4
  (`FineTableCapacity`) before CR3 activation. Replacing the arbitrary cap with
  the complete 2,048-region geometry passed `make check` and the same full
  two-vCPU QEMU regression, including all privilege-entry checks and clean
  shutdown.

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
