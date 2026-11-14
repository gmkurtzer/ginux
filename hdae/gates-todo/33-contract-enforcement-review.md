# Contract enforcement and syscall mediation review

## Source

Defensive security-review initiative; depends on Gate 32 and uses Gate 31's
filesystem foundation without reopening it.

## Goal

Verify that every managed operation is governed by the immutable effective
contract and selected dispatch model before any host effect, with fail-closed
handling of mutable inputs, unknown operations, and stale catalogs.

## Approved scope

- Review contract parsing, normalization, compilation, digesting, catalog
  versioning, selective dispatch, paranoid dispatch, denial, result translation,
  and audit behavior.
- Trace syscall arguments, pointer payloads, descriptor identity, and copied
  structures from entry snapshot through decision and completion.
- Add defensive mutation and synthetic negative tests for ordering, stale state,
  malformed inputs, and unknown operations.
- Remediate in-scope enforcement defects and add permanent regressions.

## Non-goals

- No exploit construction, escape attempts, weaponized programs, or testing of
  systems outside the controlled Ginux fixtures.
- No expansion of the direct-safe catalog solely to improve performance.
- No broad Linux ABI compatibility work unrelated to an enforcement defect.

## Completed work

- [ ] None.

## Remaining work

- [ ] Trace all enforcement routes and resolve every in-scope finding.
- [ ] Add decision-order, input-snapshot, and fail-closed regressions.
- [ ] Document residual assumptions and unsupported operation classes.

## Acceptance criteria

- Contract denial precedes hook ownership, host execution, and audit completion
  on every managed route.
- Mutable tracee inputs affecting authority are copied once and decisions do not
  re-read attacker-controlled state.
- Unknown ABI entries, catalog revisions, rules, flags, and operations fail
  closed with stable Linux-compatible results.
- Selective direct execution requires both contract permission and cataloged
  direct-safe semantics; paranoid dispatch has no direct path.
- No unresolved in-scope high- or critical-impact finding remains.

## Verification plan

- `make test-contract` -> contract compilation and runtime decisions pass.
- `make test-native-supervision` -> configured native routes preserve
  contract-before-effect ordering.
- `make test-security` -> defensive mutation and immutable-input tests pass.
- `make test` -> complete aggregate regression suite passes.
- Final route trace -> every managed syscall reaches one documented decision and
  host-effect owner.

## Evidence

- Not run.

## Decisions and limitations

- Negative verification uses controlled fixtures and state assertions, not
  exploit payloads.
- Filesystem operation-family enablement remains outside this gate and follows
  Gate 31's staged plan.

## Delivery strategy

- Working branch: `hdae/security-allocator-initiative`
- Intended base: `main`
- Pull request: one combined Gates 32-42 pull request intended
- Planned commits: route inventory; decision mutations; remediation; final
  documentation and evidence
- Actual commits: pending

## Review policy and outcome

Authorized for autonomous execution in the human-approved Gates 32-42 batch.
Keep the gate open for one final human checkpoint on the combined pull request.
