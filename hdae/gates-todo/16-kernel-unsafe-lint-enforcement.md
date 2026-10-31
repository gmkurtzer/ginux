# Gate 16: Kernel Unsafe and Lint Enforcement

## Source

GitHub issue #5, unsafe-boundary and kernel-lint finding.

## Goal

Run clippy directly on the kernel and prevent regression beyond the current
unsafe-boundary baseline while the existing unsafe code is reduced over time.

## Approved scope

- Add direct warning-denying clippy coverage for `ginux-kernel`.
- Add deterministic ratchets for unsafe blocks, SAFETY comments, and top-level
  `static mut` declarations.
- Include the ratchet in the architecture check.

## Non-goals

- Do not claim that every existing unsafe block is justified.
- Do not rewrite all unsafe code in one change.
- Do not close issue #5 or merge the pull request.

## Completed work

- [x] Added direct kernel clippy to `make check`.
- [x] Added the unsafe-boundary regression ratchet.
- [x] Integrated the ratchet into `make architecture-check`.

## Remaining work

- [ ] Run deterministic verification.
- [ ] Deliver the PR and reciprocal links.

## Acceptance criteria

- Kernel clippy runs directly with warnings denied.
- New unsafe blocks or top-level `static mut` declarations fail checks.
- Removing SAFETY annotations below the baseline fails checks.
- Existing repository checks pass.

## Verification plan

- `cargo clippy --package ginux-kernel --target x86_64-unknown-none -- -D warnings` -> passes.
- `make architecture-check` -> reports the kernel unsafe baseline and passes.
- `make check` -> passes.

## Evidence

- Baseline: 1,033 unsafe blocks, 109 SAFETY comments, one top-level `static mut`.

## Decisions and limitations

- The initial ratchet prevents regression; future gates should lower the unsafe
  ceiling and increase documented boundaries.

## Delivery strategy

- Working branch: `hdae/kernel-unsafe-lint-baseline`
- Intended base: `main`
- Pull request: intended; non-closing reference to issue #5
- Planned commits: lint and unsafe ratchet; delivery links
- Actual commits: pending

## Review policy and outcome

Delegated by the saved proactive scheduled-review policy.
