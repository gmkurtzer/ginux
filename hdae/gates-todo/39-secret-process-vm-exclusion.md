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

## Remaining work

- [ ] Validate remote ranges against authoritative per-process PTE metadata.
- [ ] Reject secret pages in all remote-copy primitives.
- [ ] Add deterministic denial and non-mutation evidence.
- [ ] Run focused, static, and complete QEMU verification.
- [ ] Push, open the remediation PR, add reciprocal links, and confirm CI.

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

## Decisions and limitations

- Secret exclusion is mapping metadata, not discretionary ptrace policy. An
  otherwise authorized caller still receives `EFAULT` for secret pages.
- The PTE is the source of truth. Copying the bit into a second mapping summary
  would create two security-sensitive representations that could diverge.

## Delivery strategy

- Working branch: `hdae/issue-24-secret-process-vm`
- Intended base: `main`
- Pull request: intended; non-closing reference to issue #24
- Planned commits: gate definition; secret metadata and remote-copy regression;
  evidence; delivery
- Actual commits: pending

## Review policy and outcome

Delegated in the user-approved remaining-reports batch; stop only if preserving
secret metadata requires a broader address-space ABI change.
