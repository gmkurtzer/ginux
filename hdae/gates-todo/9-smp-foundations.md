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
- [x] Start each discovered application processor through a read-only,
  executable low-memory trampoline, give it private guarded architecture
  state, and publish its online state only after GDT/TSS/IDT and CPU
  protections are active.
- [x] Add kernel-only fixed IPIs, remote translation invalidation, and a
  deterministic proof that all online processors execute concurrently.
- [x] Give application processors separate guarded kernel and privilege-entry
  stacks, make the Linux child trap state per CPU, and execute a scheduled
  Linux ring-3 child through an application processor.
- [x] Execute two independent Linux ring-3 address spaces simultaneously on
  the BSP and an application processor, using a bounded shared-page rendezvous
  that cannot pass through sequential placement.
- [x] Track inherited per-process CPU affinity, enforce the intersection of
  process and domain-visible online processors, and expose that mask through
  the Linux affinity syscalls.

## Remaining work

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
- Early AP-startup runs reached the SIPI trampoline but failed first at the
  temporary GDT load and then at paging activation. Instruction-stage markers
  and a QEMU exception trace identified an unintended segment-relative GDT
  address, an extra CR3 mailbox dereference, and missing AP-side EFER.NXE.
  Correcting those architectural transitions advanced the AP into the Rust
  entry point without weakening W^X.
- `cargo fmt --all`, `make architecture-check`, `make check`, and
  `make build-verification && GINUX_CPUS=2 bash tests/qemu/boot.sh` pass after
  removing the temporary diagnostics. The transcript reports
  `ginux: online processors: 2`, completes the Linux runtime matrix, and ends
  with `Boot test passed` across the initial, reboot, and corruption phases.
- The first live-IPI regression failed the Linux child lifecycle precondition
  with diagnostic bitmap 63. The user interrupt-surface verifier had treated
  the new DPL0 IPI vector as an unexpected user gate. It now accepts present
  kernel-only DPL0 gates while retaining `INT 0x80` as the sole DPL3 gate.
- `cargo fmt --all`, `make architecture-check`, `make check`, and a fresh
  `make build-verification && GINUX_CPUS=2 bash tests/qemu/boot.sh` pass after
  removing diagnostics. The transcript reports cross-CPU interrupts,
  invalidation, and concurrent execution verified, completes all three boot
  phases, and ends with `Boot test passed`.
- The first application-processor Linux child run faulted on return because
  the AP's idle continuation and TSS privilege-entry stack shared storage.
  Separating those guarded stacks removed the overwrite rather than masking
  the fault.
- A fresh two-vCPU `make build-verification && GINUX_CPUS=2
  bash tests/qemu/boot.sh` executes the lifecycle child in ring 3 on CPU 1,
  validates its complete register and FS context and exit status, completes
  all boot phases, and ends with `Boot test passed`.
- The first simultaneous-process proof encoded runtime shared-page metadata in
  a synthetic leaf mapping; both address spaces validated, but the AP faulted
  before reaching the rendezvous. The final proof temporarily aliases an
  already validated writable test page, bounds AP arrival, restores the
  original mapping before teardown, and fails closed if either participant is
  absent.
- Fresh `cargo fmt --all`, `make architecture-check`, `make check`, and
  `make build-verification && GINUX_CPUS=2 bash tests/qemu/boot.sh` pass after
  temporary diagnostics are removed. The transcript reports `concurrent Linux
  processes executed across processors`, validates lifecycle cleanup, and ends
  with `Boot test passed`.
- The first Linux-visible affinity run reached the linked userspace probe but
  failed because its regression assertion hard-coded the historical one-CPU
  mask. The probe now validates that CPU 0 remains available without assuming
  a fixed topology; the kernel verifier also restores the process mask after
  its mutation tests.
- Fresh `cargo fmt --all`, `make architecture-check`, `make check`, and
  `make build-verification && GINUX_CPUS=2 bash tests/qemu/boot.sh` pass with
  inherited process affinity and domain/online CPU-mask enforcement. The full
  three-phase transcript ends with `Boot test passed`.

## Decisions and limitations

Hotplug remains deferred. The one-vCPU path remains supported and tested even
though two vCPUs become the ordinary development default.

## Delivery strategy

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: the shared ten-gate pull request.
- Planned commits: discovery/AP startup, per-CPU state, IPIs/shootdowns,
  scheduler concurrency, domain policy, QEMU defaults, and integration evidence.
- Actual commits: `2ad2c72a` (topology identities), `beafc0f0` (per-CPU
  architecture state), `e0489201` (AP startup), `e0651591` (IPIs and remote
  invalidation), `a34b93c3` (AP Linux execution), and `7759bca7` (simultaneous
  Linux execution); CPU-affinity commit pending.

## Review policy and outcome

Delegated closure authorized after all 1/2/4-vCPU functionality, policy, stress,
and QEMU-default criteria pass. Outcome: pending.
