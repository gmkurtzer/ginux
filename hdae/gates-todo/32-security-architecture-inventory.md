# Security architecture and trust inventory

## Source

Human-directed defensive security-review initiative approved for persistence on
2026-11-14.

## Goal

Produce a complete, reviewable inventory of Ginux trust boundaries, privileged
components, authority seams, and security invariants so later reviews cannot
silently omit a host-effect path.

## Approved scope

- Inventory trusted processes, helpers, host syscalls, credentials, descriptors,
  persistent state, `unsafe` Rust, assembly, FFI, and mutable tracee-memory access.
- Map each trusted seam to an owner, invariant, existing verification, and known
  limitation.
- Trace configured launch, native supervision, OCI preparation, lifecycle,
  packaging, and recovery paths.
- Add deterministic inventory and completeness checks where practical.

## Non-goals

- No exploit development, weaponization, payload construction, or offensive
  testing against Ginux or any third-party system.
- No claim of independent certification or production readiness.
- No remediation beyond small corrections required to make the inventory
  accurate; substantive findings become later gates.

## Completed work

- [x] Inventoried installed trusted executables, host-effect routes, privilege
  transitions, persistent state, unsafe Rust, FFI, assembly, and allocator seams.
- [x] Published owner, invariant, evidence, and residual-trust matrices in
  `docs/security-assurance.md`.
- [x] Added structural inventory checks for trusted paths, privilege markers,
  package contents, and approved unsafe files.

## Remaining work

- [x] Build and validate the trust and authority inventory.
- [x] Publish the threat-to-control and seam-to-evidence matrices.
- [x] Record omissions and proposed follow-up ownership without silently fixing
  out-of-scope findings.
- [ ] Human review and acceptance of the combined pull request.

## Acceptance criteria

- Every trusted executable, privilege transition, host-effect executor, unsafe
  block, FFI/assembly boundary, and persisted security state has one documented
  owner and invariant.
- Every security invariant maps to at least one deterministic test or an explicit
  documented evidence gap.
- Generated/static checks reject an unowned newly introduced trusted seam.
- Potentially sensitive findings are represented by sanitized references until
  remediated rather than disclosed in actionable detail.

## Verification plan

- `make check` -> strict formatting, lint, and generated inventory checks pass.
- `make test-unit` -> inventory ownership and completeness tests pass.
- `make test-security` -> existing defensive boundary regressions remain green.
- Manual source trace from operator CLI to each host effect -> every path maps to
  the published inventory without an orphan.

## Evidence

- `make check` -> `SECURITY_INVENTORY: PASS`; unowned trusted-seam drift is rejected.
- `make test-unit` through aggregate `make test` -> all component ownership and
  invariant suites passed.
- `make test-security` -> all bounded defensive boundary regressions passed.
- Manual source-to-effect trace is published in `docs/security-assurance.md`;
  every installed trusted executable and documented host-effect route has an owner.

## Decisions and limitations

- This is a defensive source and architecture review of the project-owned
  codebase. Synthetic negative fixtures may demonstrate invariant failure, but
  the gate does not create or exercise exploits.
- Gate 31's filesystem foundation is authoritative for its covered seams and is
  referenced rather than duplicated.

## Delivery strategy

- Working branch: `hdae/security-allocator-initiative`
- Intended base: `main`
- Pull request: one combined Gates 32-42 pull request intended
- Planned commits: gate alignment; inventory and documentation; completeness
  checks; final evidence
- Actual commits: `94c4f4f4`, `36c742e5`, `2163caa3`

## Review policy and outcome

Technically passed under the authorized batch. Kept open for the final human
checkpoint on the combined pull request.
