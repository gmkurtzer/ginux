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

- [x] Run deterministic verification.
- [x] Push the existing branch and update pull request #9.

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
- `make architecture-check` and `make check` -> passed.

## Decisions and limitations

- The initial ratchet prevents regression; future gates should lower the unsafe
  ceiling and increase documented boundaries.
- The user explicitly directed the corrective work into existing pull request
  #9, overriding the default one-gate-per-PR delivery split.

## Delivery strategy

- Working branch: `codex/issue-5-criticism-reconciliation`
- Intended base: `main`
- Pull request: existing PR #9
- Planned commits: lint and unsafe ratchet; delivery links
- Actual commits: `476fba04` (lint and unsafe ratchet); delivery checkpoint pending

## Review policy and outcome

Delegated by the saved proactive scheduled-review policy; remains open for
maintainer review in pull request #9.
