# Gate 36: Per-Domain Linux Memory Accounting

## Source

[GitHub issue #19](https://github.com/gmkurtzer/ginux/issues/19).

## Goal

Charge Linux runtime memory to its container domain and reject further mmap,
stack, copy-on-write, tmpfs, and SysV shared-memory growth at the configured
`memory_limit_mib` instead of exhausting the global kernel pool.

## Approved scope

- Add a configurable per-owner allocation limit while retaining the existing
  fixed service-domain frame ceiling.
- Derive a container frame owner and frame limit from its generation-safe
  domain ID and `memory_limit_mib`.
- Route private Linux runtime, tmpfs, and SysV IPC allocations and releases
  through that domain policy.
- Add deterministic allocator and runtime accounting regressions, including
  independent mmap-style and SysV allocation exhaustion.

## Non-goals

- Do not redesign virtual-address `RLIMIT_AS`; it remains a separate Linux ABI
  limit layered above physical domain accounting.
- Do not charge globally shared immutable file-cache pages to one consumer.
- Do not merge the PR or close issue #19.

## Completed work

- [x] Confirmed `memory_limit_mib` has no allocator consumer.
- [x] Confirmed dynamic Linux runtime, tmpfs, and SysV frames are currently
  allocated as `FrameOwner::KERNEL`.
- [x] Confirmed the allocator's existing 192-frame non-kernel ceiling is a
  service-domain policy and cannot represent configurable container limits.

## Remaining work

- [ ] Add configurable owner-limit allocation without weakening service quotas.
- [ ] Charge and release Linux private runtime frames to the container owner.
- [ ] Charge and release per-container tmpfs and SysV IPC frames.
- [ ] Add deterministic quota and syscall-family regressions.
- [ ] Run focused, static, and complete QEMU verification.
- [ ] Push, open the remediation PR, add reciprocal links, and confirm CI.

## Acceptance criteria

- Container-owned frames cannot exceed the configured MiB-to-page limit.
- Exhaustion fails the initiating mmap/touch or SysV allocation without
  consuming another domain's budget or the unbounded kernel-owner pool.
- Released frames restore the owning domain's capacity and stale generations
  remain rejected.
- Existing supervised-service frame ceilings and restart/quarantine behavior
  remain unchanged.
- Static and complete QEMU verification pass.

## Verification plan

- Allocator regression -> a configurable owner stops exactly at its frame
  limit, another owner remains independent, release restores capacity, and the
  legacy service ceiling remains unchanged.
- Linux runtime proof -> private runtime and SysV allocations are owned by the
  current container and independently return `ENOMEM` at its configured limit.
- Source inspection -> mmap/stack/COW/tmpfs/SysV allocation and release paths
  use the domain policy; shared immutable cache pages remain kernel-owned.
- `make check` and `make test` -> full static and boot checks pass.

## Evidence

- Issue #19's static report matches the current allocation paths and the
  architecture's existing requirement that a domain own and account memory.

## Decisions and limitations

- `memory_limit_mib` is a physical frame budget for private domain-owned memory;
  `RLIMIT_AS` continues to constrain virtual address space independently.
- Global immutable file-cache frames remain kernel-owned because assigning a
  shared cache page to one domain would make ownership depend on lookup order.

## Delivery strategy

- Working branch: `hdae/issue-19-domain-memory`
- Intended base: `main`
- Pull request: intended; non-closing reference to issue #19
- Planned commits: gate definition; configurable allocator budget; Linux
  runtime ownership; tmpfs and SysV ownership; regression/evidence; delivery
- Actual commits: pending

## Review policy and outcome

Delegated in the user-approved remaining-reports batch. The architecture
already requires domain memory ownership; stop for any implementation result
that would require changing that policy or weakening an existing service quota.
