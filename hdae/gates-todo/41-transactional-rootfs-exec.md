# Gate 41: Make Rootfs Exec Transactional

## Source

[GitHub issue #27](https://github.com/gmkurtzer/ginux/issues/27).

## Goal

Make every failed rootfs `exec` leave the complete old process image and frame
ownership intact, while a successful `exec` publishes one complete new image.

## Approved scope

- Build the replacement address space and complete process-image metadata in
  staged ownership that is not observable by the running process.
- Materialize ELF and stack bytes directly into their final replacement frames;
  staging must not add a payload copy.
- Publish the mutually consistent replacement at one commit point, then retire
  the old image exactly once.
- Cover page tables, mapping registry, heap/stack, mmap state, TLS,
  thread/process context, descriptors, signals, vvar, and frame ownership.

## Non-goals

- Do not weaken ELF, W^X, resource, or entry validation.
- Do not make a failed exec partially successful.
- Do not merge the PR or close issue #27.

## Completed work

- [x] Confirmed segment PTEs, mapping registry, and active-image frame metadata
  are replaced before fallible heap/stack/reset/release/context checks.
- [x] Confirmed old segment frames may be scrubbed before those late checks.
- [x] Identified all late mutable surfaces that must share one transaction.
- [x] Human selected staged replacement address-space publication.

## Remaining work

- [x] Implemented staged allocation/materialization and the hierarchy primitive's
  deterministic forced pre-commit rollback check.
- [x] Ran focused, static, and complete deterministic QEMU verification.
- [ ] Obtain a reachable OCI registry/cache and rerun the Rocky shell path so
  the rootfs-specific integration branch executes end to end.
- [x] Pushed, opened PR #53, and added the reciprocal issue link. CI remains
  external follow-up evidence.

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
- `make check` -> passed, including architecture, compatibility, clippy, and
  all host-side crate tests.
- `make test` -> passed the complete multi-boot QEMU sequence; the common exec
  hierarchy transaction forced a preparation failure before commit, retained
  the old root, and then committed the staged hierarchy successfully.
- `make test-rocky-shell` -> review incomplete before the code under test: the
  guest rejected every OCI registry DNS response and exhausted three pull
  attempts, so no image was unpacked or executed.
- Synchronization with `main` at `b4628fc2` -> the staged transaction now uses
  the domain-aware runtime frame policy for allocation and rollback; the only
  code conflicts were this ownership integration and retaining both QEMU
  transaction assertions.
- Fresh `make check` and complete `make test` after synchronization -> passed,
  including `GINUX_KERNEL_OK`, the staged hierarchy transaction assertion,
  bounded OCI unpack assertion, and `Boot test passed`.
- Fresh `make test-rocky-shell` after synchronization -> failed before the
  code under test for the same external reason: all three registry DNS
  resolutions failed, no image was downloaded, unpacked, or executed.
- Synchronization with current `main` at `c8d12653` -> merged cleanly except
  for the active HDAE checkpoint; `make check` and complete `make test` pass,
  including the staged hierarchy transaction proof and controller-desktop
  regressions.
- Fresh `make test-rocky-shell` after that synchronization -> DHCP succeeded,
  but all three Docker registry DNS resolutions failed before image download,
  unpack, or rootfs `exec`; the required integration path remains not run.
- PR #53: <https://github.com/gmkurtzer/ginux/pull/53>.
- Issue summary: <https://github.com/gmkurtzer/ginux/issues/27#issuecomment-6569276429>.

## Decisions and limitations

- The human selected staged address-space publication because it keeps failure
  cleanup local, reuses the existing replacement-hierarchy machinery, and adds
  no payload copy. Temporary peak ownership includes old plus replacement
  frames until commit.
- Default deterministic QEMU does not enter the OCI rootfs exec branch. The
  common hierarchy transaction is exercised, but rootfs-specific integration
  remains blocked on a reachable registry or populated cache.

## Delivery strategy

- Working branch: `hdae/issue-27-transactional-rootfs-exec`
- Intended base: `main`
- Pull request: intended after the design choice; non-closing reference to #27
- Planned commits: gate definition; transaction mechanism and failure
  injection; evidence; delivery
- Actual commits: `cb87d157` defines the gate; `8a952b29` implements staged
  publication and deterministic transaction verification; `65ac3d0d` records
  evidence; `85950463` merges current `main`; final delivery evidence is
  pending the reachable-registry integration run.

## Review policy and outcome

The transaction-boundary checkpoint was resolved by the human. Implementation
is complete and deterministic evidence passes, but the gate remains partial
until the rootfs-specific Rocky integration path can run. Publishing the PR and
reciprocal comments is authorized; merge and issue closure are not.
