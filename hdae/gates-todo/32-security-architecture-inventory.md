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

- [ ] None.

## Remaining work

- [ ] Build and validate the trust and authority inventory.
- [ ] Publish the threat-to-control and seam-to-evidence matrices.
- [ ] Record omissions and proposed follow-up ownership without silently fixing
  out-of-scope findings.

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

- Not run.

## Decisions and limitations

- This is a defensive source and architecture review of the project-owned
  codebase. Synthetic negative fixtures may demonstrate invariant failure, but
  the gate does not create or exercise exploits.
- Gate 31's filesystem foundation is authoritative for its covered seams and is
  referenced rather than duplicated.

## Delivery strategy

- Working branch: `hdae/security-architecture-inventory`
- Intended base: `main`
- Pull request: intended
- Planned commits: gate alignment; inventory and documentation; completeness
  checks; final evidence
- Actual commits: pending

## Review policy and outcome

Security checkpoint requiring explicit human acceptance. Persistence of this
gate does not authorize implementation.
