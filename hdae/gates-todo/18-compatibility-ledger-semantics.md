# Gate 18: Compatibility Ledger Semantics

## Source

GitHub issue #5, compatibility-ledger finding.

## Goal

Make compatibility status labels reflect observable behavior: obsolete
ENOSYS-only syscalls are not Complete, and bounded socket calls are not Complete
while the documented socket-family contract remains partial.

## Approved scope

- Add an explicit Unsupported status for pinned x86-64 entries that
  intentionally return Linux-compatible `-ENOSYS`.
- Reclassify the 17 ENOSYS-only entries dispatched explicitly by the kernel.
- Reclassify bounded socket calls from Complete to Partial.
- Update the deterministic ledger checker and totals.

## Non-goals

- Do not implement new syscall or socket behavior.
- Do not change syscall dispatch or ABI results.
- Do not close issue #5 or merge the pull request.

## Completed work

- [x] Added the Unsupported status and checker support.
- [x] Reclassified 17 explicit ENOSYS-only entries.
- [x] Reclassified 12 bounded socket calls as Partial.
- [x] Kept Unsupported entries in the generated runtime catalog as expected
  failures rather than silently omitting them.

## Remaining work

- [x] Run deterministic verification.
- [x] Push the branch, open the PR, and add reciprocal links.

## Acceptance criteria

- No explicitly dispatched ENOSYS-only entry is labeled Complete.
- Socket calls covered by documented family limitations are Partial.
- Declared totals equal the matrix and the checker passes.
- Repository static checks pass without behavior changes.

## Verification plan

- `make linux-compat-check` -> reports 53 Complete, 257 Partial, 48
  Policy-denied, 17 Unsupported, and 0 Missing.
- `make architecture-check` -> passes.
- `make check` -> passes.

## Evidence

- Current dispatch explicitly returns `LINUX_ENOSYS` for the 17 reclassified
  syscall numbers.
- The socket-family boundary documents incomplete ancillary, IPv6, wait,
  backlog, and option semantics.
- `make linux-compat-check` -> passed with 53 Complete, 257 Partial, 48
  Policy-denied, 17 Unsupported, and 0 Missing.
- `make architecture-check` and `make check` -> passed.
- Pull request: https://github.com/gmkurtzer/ginux/pull/11
- Source response: https://github.com/gmkurtzer/ginux/issues/5#issuecomment-6540094116
- Reciprocal PR comment: https://github.com/gmkurtzer/ginux/pull/11#issuecomment-6540093930

## Decisions and limitations

- Unsupported distinguishes intentional Linux-compatible ENOSYS behavior from
  an unimplemented default-dispatch hole.

## Delivery strategy

- Working branch: `hdae/compatibility-ledger-semantics`
- Intended base: `main`
- Pull request: intended; non-closing reference to issue #5
- Planned commits: ledger semantics and verification; delivery links
- Actual commits: `58923752` (ledger, checker, runtime catalog, and gate)

## Review policy and outcome

Delegated by the saved proactive scheduled-review policy. All criteria passed,
and delivery is complete through PR #11 with reciprocal links. The gate remains
open for maintainer review.
