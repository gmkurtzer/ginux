# Gate 39: Preserve Secret Memory Exclusion Across Processes

## Source

[GitHub issue #24](https://github.com/gmkurtzer/ginux/issues/24).

## Goal

Prevent `process_vm_readv`, `process_vm_writev`, and other kernel remote-copy
helpers from reading or modifying a live process's `memfd_secret` pages.

## Approved scope

- Enforce the authoritative hardware runtime-page secret attribute for live
  process ranges instead of trusting the lossy imported mapping summary.
- Reject secret pages before every direct-map remote read or write.
- Add deterministic read-denial, write-denial, and non-mutation regressions.
- Retain ordinary cross-process transfer, partial-transfer, permission, and
  error behavior.

## Non-goals

- Do not change ptrace credential policy or grant new debugging authority.
- Do not change the local userspace accessibility semantics of `memfd_secret`.
- Do not merge the PR or close issue #24.

## Completed work

- [x] Confirmed current PTE runtime metadata retains the secret bit.
- [x] Confirmed `current_linux_active_mapping` omits that bit.
- [x] Confirmed remote-copy helpers dereference the frame direct map without a
  secret-page check.
- [x] Made the target process PTE secret bit authoritative for remote range,
  read, write, COW-preparation, scalar-write, and console-copy paths.
- [x] Added a cross-process regression that denies both syscalls and helper
  calls, clears the test bit, and proves the original target bytes are intact.
- [x] Passed the complete static and QEMU regression suites.
- [x] Published PR #48 and added reciprocal issue/PR summaries.

## Remaining work

- [ ] GitHub CI is external evidence and must finish before merge.

## Acceptance criteria

- Imported and forked secret mappings remain excluded because remote access
  consults their authoritative PTE secret bit.
- Remote reads and writes spanning a secret page fail without disclosing or
  modifying any byte of that page.
- Ordinary remote reads/writes, partial transfers, and credential checks remain
  unchanged.
- Static and complete QEMU verification pass.

## Verification plan

- Mapping lifecycle contract -> forked/imported mappings retain the PTE secret
  bit and remote access consults that authoritative bit.
- Remote-copy regression -> read, write, and mapped-range checks reject a
  secret mapping, and its original bytes remain unchanged.
- Existing cross-process verifier -> ordinary success, partial transfer,
  permission denial, and invalid-range behavior still pass.
- `make check` and `make test` -> full static and boot checks pass.

## Evidence

- Issue #24's cited paths match `kernel/src/arch/x86_64/cpu.rs`,
  `kernel/src/address_space.rs`, and `kernel/src/linux_process.rs` on current
  `main`.
- `make check` passed, including the reduced 1,032-block unsafe inventory and
  unchanged Linux compatibility matrix.
- `make test` passed and emitted
  `secret process memory excluded from remote copy`.
- The regression returned `EFAULT` for remote read and write, rejected direct
  helper access, and observed byte-for-byte target equality after clearing the
  verification-only secret bit.

## Decisions and limitations

- Secret exclusion is mapping metadata, not discretionary ptrace policy. An
  otherwise authorized caller still receives `EFAULT` for secret pages.
- The PTE is the source of truth. Copying the bit into a second mapping summary
  would create two security-sensitive representations that could diverge.

## Delivery strategy

- Working branch: `hdae/issue-24-secret-process-vm`
- Intended base: `main`
- Pull request: [#48](https://github.com/gmkurtzer/ginux/pull/48), with a
  non-closing reference to issue #24
- Planned commits: gate definition; secret metadata and remote-copy regression;
  evidence; delivery
- Actual commits: `0999760d` (gate definition), `653b9c44` (authoritative PTE
  exclusion and regression), plus the evidence/delivery commit containing this
  record

## Review policy and outcome

Passed and closed under the user-approved remaining-reports batch delegation.
No address-space ABI change was required.
