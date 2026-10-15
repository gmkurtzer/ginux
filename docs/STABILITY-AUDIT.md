# Runtime stability audit

Status: original S0 runtime-isolation findings closed; broader C3 compatibility
and deterministic registry coverage remain active.

## Executive finding

Ginux now models the executable Linux runtime as a domain-owned object. Runtime
extents, mutable ELF/Linux ABI state, root/mount/process views, retained CPU
state, and console continuations are selected by domain. Detach schedules the
resident PID-1 controller instead of recursively relaunching it. The concurrent
Alpine/Rocky QEMU gate is the release check for these invariants.

## Severity-zero invariants

These are release-blocking. A checkpoint must not claim multi-domain support
until every invariant has an automated end-to-end test.

1. **A Linux runtime belongs to one domain.** Its page tables, image mappings,
   heap, mmap state, TLS, descriptors, CWD, signals, threads, process table,
   mount namespace, tmpfs, and console continuation cannot be selected through
   a host-wide singleton.
2. **A running domain owns its memory until it exits.** OCI runtimes cannot
   share the reusable bootstrap runtime extent.
3. **Detach is a scheduler operation.** It must save a resumable process context
   and return to the resident controller. It must not recursively invoke a new
   controller on the blocked container's kernel stack.
4. **The controller is resident.** Starting, attaching, stopping, or faulting a
   container must not restart the controller or change controller PID 1.
5. **Domain failure is contained.** A failed syscall, process, driver, pull,
   or init changes only the owning domain's lifecycle state. It must not panic
   the kernel or stop another container.
6. **Process state is process-owned.** Fork/exec must preserve the parent and
   isolate child mmap, brk, descriptors, CWD, credentials, signals, TLS, and
   image metadata. Restoring only mmap metadata is not sufficient.

## Original defects and closure

| Severity | Defect | Evidence | Required closure |
| --- | --- | --- | --- |
| S0 | Only one domain can execute | A second start was rejected | Closed: retained per-domain runtime records and scheduler bindings |
| S0 | OCI memory is singleton | A reusable extent was shared | Closed: domain-owned allocations survive suspension until teardown |
| S0 | Detach is recursive | Console detach relaunched the controller | Closed: save context, block the domain, schedule resident PID 1 |
| S0 | Linux ABI state is singleton | ELF, mmap, descriptor, signal, thread, CWD, and TLS state was host-wide | Closed: mutable state is domain-banked and process-owned where implemented |
| S0 | Fork/exec can leak child state | Parent mmap state was replaced | Closed for the current bounded ABI profile; compatibility expansion remains test-driven |
| S1 | Host process inspection models one fake init | Host inspection synthesized one PID | Closed: enumerate running domain init records with stable host PIDs |
| S1 | Start failures are misreported | all rejected starts say `image not ready` | Return and display typed lifecycle errors |
| S1 | Live OCI gate is nondeterministic | registry DNS failure prevented two full Alpine test runs | Deterministic registry fixture plus optional public-registry test |
| S1 | No two-domain QEMU gate exists | The Alpine script tested one domain sequentially | Closed: `make test-multi-domain` runs the sequence below |
| S2 | Resource enforcement is incomplete | configured CPU and arbitrary nonzero 64-bit MiB values are retained and reported, but allocations are not yet charged to domains | Enforce the configured values through domain allocation accounting |
| S2 | Roadmap/checkpoint claims are ahead of verification | C3 text describes behavior absent from the QEMU gate | Derive status from passing gates, not implementation intent |

## Required end-to-end acceptance sequence

The multi-domain test must boot a clean image and perform this sequence over
the real controller console:

1. Create Alpine and Rocky Linux personality domains with explicit image references and
   `/bin/sh` init.
2. Start Alpine detached; verify pull, digest, architecture selection, unpack,
   domain creation, and `running` state.
3. Start Rocky detached while Alpine remains running; verify the same stages.
4. Verify `domain list`, host `/proc`, and `ps` report both domain init
   processes with distinct stable identities.
5. Attach Alpine, run filesystem/process ABI probes, detach, and confirm Rocky
   remains running.
6. Attach Rocky, run the same probes, detach, and confirm Alpine remains
   running.
7. Stop and restart each domain independently.
8. Exit one init and inject a fault into the other; verify correct per-domain
   state and a responsive PID-1 controller.
9. Shut down cleanly with no kernel panic and no leaked domain-owned frames.

The test is not satisfied by lifecycle-state simulation. Both init processes
must retain independent address spaces and resumable execution contexts.

## Verification record

The historical baseline at the start of this audit was:

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

The domain-first control-plane iteration was verified on 2026-10-15:

- `make check`: pass.
- `make test-c1`: pass.
- `make test-alpine`: pass, including live registry ingestion, attach/detach,
  clean init exit, and restart.
- `make test-multi-domain`: pass, including concurrent Alpine and Rocky domain
  lifecycle and independent attach/stop behavior.
- `make test`: pass. The apparent interpreted-loader fault was a deliberate
  probe failure caused by an obsolete all-or-nothing `getdents64` expectation;
  the probe now verifies Linux-compatible whole-record partial reads and a
  genuinely undersized first-record buffer.

## Stabilization progress

- OCI runtime extents are now allocated per domain slot. Starting another
  domain can no longer reuse the first domain's runtime pages.
- Mutable Linux/ELF ABI state is now banked by the executing domain,
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

The S0 scheduler item is closed by retained per-domain contexts and resident
controller scheduling. The selected/active slot variables now identify the
domain currently executing on the serialized POC CPU; they are not ownership
of the other suspended domains' runtime state.
