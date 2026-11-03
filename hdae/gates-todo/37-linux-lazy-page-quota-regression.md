# Gate 37: Linux Lazy-Page Quota Regression

## Source

[GitHub issue #57](https://github.com/gmkurtzer/ginux/issues/57).

## Goal

Restore Alpine and Rocky Linux startup after per-domain frame accounting so a
valid demand-zero anonymous mapping can materialize its first writable page
within the configured domain memory budget.

## Approved scope

- Diagnose the exact allocation failure after the first anonymous `mmap`.
- Correct the domain-owned lazy-page allocation or accounting path without
  bypassing `memory_limit_mib`.
- Add a deterministic regression that reaches the failing allocation boundary.
- Verify the console-mode Alpine integration path and the complete repository
  regression suites.

## Non-goals

- Do not change Wayland, graphical-controller, or OCI image semantics.
- Do not weaken or remove per-domain physical-frame limits.
- Do not alter PR #56's native network-clock design.
- Do not merge the remediation PR during its first validation pass.

## Completed work

- [x] Reproduced issue #57 with `make test-alpine` on current `main` plus the
  reviewed PR #56 head.
- [x] Confirmed OCI pull, verification, unpack, and publication complete before
  the failure.
- [x] Confirmed Linux init faults on the first writable anonymous page at
  `0x8000000000` with an occupied demand mapping and no materialized frame.
- [x] Confirmed Wayland is not involved and posted the evidence to issue #57.
- [x] Replaced payload-frame lookups that still assumed kernel ownership with
  generation-safe domain/kernel/runtime recognition while retaining kernel-only
  ownership for page-table hierarchy frames.
- [x] Preserved immutable runtime-backed mappings during child exec validation,
  accepted domain-owned RELRO pages, and released private child frames through
  their recorded owner.
- [x] Added a boot-time domain-owned runtime-page metadata regression.
- [x] Passed `make check`, `make test`, and `make test-alpine`.

## Remaining work

- [ ] Make signed commits, push the dedicated branch, open the linked PR, and
  record its stable URL and first-pass evidence.

## Acceptance criteria

- A default 128 MiB Alpine domain reaches its attached `/bin/sh` instead of
  failing at Linux init stage 7.
- The first writable anonymous mapping materializes a zeroed private frame
  owned and charged to the current container domain.
- Domain exhaustion still fails closed at `memory_limit_mib`, remains isolated
  between domains, and restores capacity after release.
- Existing service-domain frame ceilings and shared immutable cache ownership
  remain unchanged.
- Static, complete QEMU, and Alpine integration verification pass.

## Verification plan

- Targeted accounting regression -> the same owner state reached by initial
  Linux exec can allocate and release the first demand-zero mmap page below
  the configured domain limit.
- Existing domain quota proofs -> exhaustion, owner isolation, and release
  restoration remain green.
- `make check` -> static, architecture, ABI, lint, and unit checks pass.
- `make test` -> complete deterministic QEMU suite passes.
- `make test-alpine` -> OCI pull/cache, Linux init, shell probes, restart, and
  failure containment pass under TCG.

## Evidence

- Issue #57 supplied a page-fault trace at `0x8000000000` after OCI cache
  restore and Linux-init entry.
- Local `make test-alpine` reproduced the exact vector 14/error 6 fault with
  `occupied=1`, `frame=0`, `file=0`, and `protection=3`, followed by stage-7
  failure.
- PR #56 does not resolve the defect; the same failure occurred with its exact
  reviewed head merged locally with current `main`.
- The first fault was not quota exhaustion: allocation succeeded, but the leaf
  decoder rejected the domain-owned frame and rolled it back as absent.
- Follow-on Alpine probes exposed the same stale ownership assumption in fork
  snapshots, exec/runtime validation, RELRO protection, and child cleanup.
- `make check` passed with the unsafe inventory unchanged at 1,033 blocks.
- `make test` passed the complete two-vCPU QEMU boot and Linux runtime suite.
- `make test-alpine` passed its OCI shell, restart, and failure-containment
  checks with the default 128 MiB domain budget.

## Decisions and limitations

- This is a console-mode Linux runtime regression, not a Wayland requirement.
- Live registry access is external evidence; the deterministic regression must
  cover the allocator/accounting boundary even when the floating Alpine image
  or registry is unavailable.
- Page-table hierarchy ownership remains kernel-only. Only payload leaves use
  the owner-agnostic live-lease decoder, and release still requires the exact
  recorded owner and generation.

## Delivery strategy

- Working branch: `hdae/issue-57-linux-lazy-page`
- Intended base: `main` at `82df3d719e14246233d509799ad220c4c39e7088`
- Pull request: intended; non-closing reference to issue #57
- Planned commits: gate and reproduction checkpoint; allocator/accounting fix
  with regression; verification and delivery evidence
- Actual commits: pending

## Review policy and outcome

Delegated by the saved hourly remediation policy. Stop for any correction that
would weaken domain quotas or change the product's memory-ownership model. A
passing delivered head receives a first-pass marker and waits for a subsequent
hourly merge check.
