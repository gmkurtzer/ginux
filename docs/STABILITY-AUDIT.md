# Runtime stability audit

Status: active stabilization work. This document records observed behavior, the
kernel invariant behind it, and the acceptance test required before the item is
closed.

## Executive finding

Ginux has a real domain model, per-container configuration, rootfs storage,
tmpfs instances, and process-table instances. The executable Linux runtime is
not yet a per-domain object. It still uses a single active container selector,
a single reusable OCI memory extent, singleton ELF/Linux ABI state, and a
recursive controller handoff while a detached shell waits for input.

Those implementation choices are the common cause of the recent regressions.
Removing only the active-container check would let the second container erase
or inherit the first container's runtime state. It is therefore not an
acceptable fix.

## Severity-zero invariants

These are release-blocking. A checkpoint must not claim multi-container support
until every invariant has an automated end-to-end test.

1. **A Linux runtime belongs to one domain.** Its page tables, image mappings,
   heap, mmap state, TLS, descriptors, CWD, signals, threads, process table,
   mount namespace, tmpfs, and console continuation cannot be selected through
   a host-wide singleton.
2. **A running container owns its memory until it exits.** OCI runtimes cannot
   share the reusable bootstrap runtime extent.
3. **Detach is a scheduler operation.** It must save a resumable process context
   and return to the resident controller. It must not recursively invoke a new
   controller on the blocked container's kernel stack.
4. **The controller is resident.** Starting, attaching, stopping, or faulting a
   container must not restart the controller or change controller PID 1.
5. **Container failure is contained.** A failed syscall, process, driver, pull,
   or init changes only the owning domain's lifecycle state. It must not panic
   the kernel or stop another container.
6. **Process state is process-owned.** Fork/exec must preserve the parent and
   isolate child mmap, brk, descriptors, CWD, credentials, signals, TLS, and
   image metadata. Restoring only mmap metadata is not sufficient.

## Confirmed defects

| Severity | Defect | Evidence | Required closure |
| --- | --- | --- | --- |
| S0 | Only one container can execute | `CONTAINER_ACTIVE_SLOT` rejects a second start | Replace with per-domain runtime records and scheduler bindings |
| S0 | OCI memory is singleton | `allocate_runtime_extent::<440>()` returns the same extent | Domain-owned allocations retained until process teardown |
| S0 | Detach is recursive | console input calls `run_installed_interactive_controller()` | Save context, block the domain, schedule the controller |
| S0 | Linux ABI state is singleton | ELF, mmap, descriptor, signal, thread, CWD, and TLS atomics are host-wide | Move state into domain/process runtime records |
| S0 | Fork/exec can leak child state | the parent mmap regression was one instance of global state replacement | Process-state isolation test covering every mutable ABI subsystem |
| S1 | Host process inspection models one fake init | host inspection synthesizes PID 100 for one active slot | Enumerate domain/process registries with stable identities |
| S1 | Start failures are misreported | all rejected starts say `image not ready` | Return and display typed lifecycle errors |
| S1 | Live OCI gate is nondeterministic | registry DNS failure prevented two full Alpine test runs | Deterministic registry fixture plus optional public-registry test |
| S1 | No two-container QEMU gate exists | current Alpine script tests one container sequentially | Add the acceptance sequence below to the required gate |
| S2 | Resource settings are not fully effective | current admission path assumes one CPU and 128 MiB | Enforce configured values through domain policy and report them |
| S2 | Roadmap/checkpoint claims are ahead of verification | C3 text describes behavior absent from the QEMU gate | Derive status from passing gates, not implementation intent |

## Required end-to-end acceptance sequence

The multi-container test must boot a clean image and perform this sequence over
the real controller console:

1. Create Alpine and Rocky containers with explicit image references and
   `/bin/sh` init.
2. Start Alpine detached; verify pull, digest, architecture selection, unpack,
   domain creation, and `running` state.
3. Start Rocky detached while Alpine remains running; verify the same stages.
4. Verify `container list`, host `/proc`, and `ps` report both domain init
   processes with distinct stable identities.
5. Attach Alpine, run filesystem/process ABI probes, detach, and confirm Rocky
   remains running.
6. Attach Rocky, run the same probes, detach, and confirm Alpine remains
   running.
7. Stop and restart each container independently.
8. Exit one init and inject a fault into the other; verify correct per-domain
   state and a responsive PID-1 controller.
9. Shut down cleanly with no kernel panic and no leaked domain-owned frames.

The test is not satisfied by lifecycle-state simulation. Both init processes
must retain independent address spaces and resumable execution contexts.

## Verification baseline

At the start of this audit:

- `make check`: pass.
- `make test-c1`: pass.
- `make test`: pass.
- `make test-alpine`: the latest run completed every boot-time subsystem and
  reached the controller, then live Docker registry DNS resolution failed;
  container execution could not begin.
- Manual two-container test: fail; the second start is rejected while the first
  container is detached.

These results show that the existing green gates do not cover the failing
runtime topology.

## Stabilization progress

- OCI runtime extents are now allocated per container slot. Starting another
  domain can no longer reuse the first domain's runtime pages.
- Mutable Linux/ELF ABI state is now banked by the executing container domain,
  with a separate host/proof instance. Controller handoff temporarily selects
  the host bank.
- `make check`, the normal `make test` boot, and `make test-c2` pass with these
  ownership changes.
- The native-volume recovery proof no longer requires exactly one timeout from
  a successful normal I/O. Interrupt completion and any bounded number of
  timeout/retry cycles are accepted only when the final outcome, lifecycle
  event, frame cleanup, DMA scrub, device quiescence, capability revocation,
  and IOMMU retirement all validate. Repeated test runs now reach the
  controller instead of panicking on host timing.

The S0 scheduler item remains open. `CONTAINER_ACTIVE_SLOT` and recursive
console handoff are intentionally still enforcing one executable Linux runtime;
lifting that guard before resumable domain contexts exist would misreport
concurrency and risk corrupting process state.
