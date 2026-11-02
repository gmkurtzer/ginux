# Gate 41: Make Rootfs Exec Transactional

## Source

[GitHub issue #27](https://github.com/gmkurtzer/ginux/issues/27).

## Goal

Make every failed rootfs `exec` leave the complete old process image and frame
ownership intact, while a successful `exec` publishes one complete new image.

## Scope awaiting human decision

Choose one complete transaction boundary before implementation:

- Recommended: build a staged replacement address space and process-image
  metadata, then publish it at one atomic commit point.
- Alternative: retain a complete old-image ownership guard and roll back page
  tables, mapping registry, heap/stack, mmap state, TLS, thread/process context,
  descriptors, signals, vvar, and all frame ownership on every late failure.

Both designs must cover every mutable surface; a segments-only rollback is not
acceptable.

## Non-goals

- Do not weaken ELF, W^X, resource, or entry validation.
- Do not make a failed exec partially successful.
- Do not merge the PR or close issue #27.

## Completed work

- [x] Confirmed segment PTEs, mapping registry, and active-image frame metadata
  are replaced before fallible heap/stack/reset/release/context checks.
- [x] Confirmed old segment frames may be scrubbed before those late checks.
- [x] Identified all late mutable surfaces that must share one transaction.

## Remaining work

- [ ] Obtain the human's transaction-boundary choice.
- [ ] Finalize acceptance and implementation tasks for that design.
- [ ] Implement deterministic injected-failure rollback checks at each stage.
- [ ] Run focused, static, and complete QEMU verification.
- [ ] Push, open the remediation PR, add reciprocal links, and confirm CI.

## Acceptance criteria

- Every injected pre-commit failure preserves the old executable bytes,
  mappings, heap/stack, mmap tree, TLS, contexts, descriptors, signals, vvar,
  and live-frame count.
- No new-image PTE or process metadata is observable before the commit point.
- Success publishes a mutually consistent new page table, registry, context,
  and resource state and releases the retired image exactly once.
- Existing exec, fork/exec, compatibility, static, and complete QEMU checks
  pass.

## Verification plan

- Deterministic failure injection at each transaction stage -> old image and
  frame inventory match the pre-exec snapshot exactly.
- Successful replacement -> hardware mappings, registry, entry/context, and
  active-image metadata all identify the same new frames.
- Existing exec and fork/exec proofs -> no behavior regression.
- `make check` and `make test` -> full static and boot checks pass.

## Evidence

- Issue #27's cited replacement sequence matches current
  `linux_rootfs_execve_source`.
- Existing helpers provide local mapping rollback and hierarchy replacement,
  but no transaction currently spans all rootfs-exec state.

## Decisions and limitations

- Open decision: staged address-space publication (recommended) versus a
  complete old-image rollback guard.
- Work is paused because choosing between these changes the ownership and
  publication architecture.

## Delivery strategy

- Working branch: `hdae/issue-27-transactional-rootfs-exec`
- Intended base: `main`
- Pull request: intended after the design choice; non-closing reference to #27
- Planned commits: gate definition; transaction mechanism and failure
  injection; evidence; delivery
- Actual commits: pending

## Review policy and outcome

Human checkpoint for the transaction-boundary choice; implementation may then
run under the approved remaining-reports batch policy.
