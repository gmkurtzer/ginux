# Bounded getrandom short reads

## Source
[Issue #163](https://github.com/gmkurtzer/ginux/issues/163)

## Goal
Enforce the balanced profile's 256-byte `getrandom` bound on the buffer length while preserving Linux-compatible short-read behavior for larger requests.

## Approved scope
- Correct the argument policy to bound `buflen`, not flags.
- Clamp larger requests to 256 bytes before the host syscall in configured launch and native supervision.
- Add focused unit and launch/native regression coverage.

## Non-goals
- Change the kernel's validation of `getrandom` flags.
- Intercept vDSO randomness or change process-native behavior.

## Completed work
- [x] Correct the policy and supervisor register handling.
- [x] Add deterministic regression coverage.
- [ ] Verify, commit, push, open the PR, and add reciprocal links.

## Remaining work
- [ ] Human review and merge.

## Acceptance criteria
- `balanced_arguments_allowed(318, ..., 256, flags)` accepts the request and a length above 256 does not pass unchanged.
- A 512-byte request returns a 256-byte short read in both launch and native fixtures.
- Existing checks remain green.

## Verification plan
- `cargo test -p ginux-core` -> argument-policy unit tests pass.
- `make check` -> formatting, build, clippy, documentation, and static checks pass.
- `sudo make test-launch test-native-supervision` on Linux CI -> both real supervision paths report a 256-byte result.

## Evidence
- `cargo test -p ginux-core` -> 9 passed.
- `make test-unit` -> passed across all unit-test crates.
- `make check` -> passed.
- x86-64 Linux-target compile of both C fixtures with strict warnings -> passed.
- Local launch/native runtime execution -> not run; the macOS arm64 host cannot execute the Linux x86-64 supervisor integration, so hosted CI remains mandatory.

## Decisions and limitations
- Clamping retains the per-call resource bound without converting a legal larger request into `EPERM`.
- vDSO calls remain outside ptrace interception, as documented by the source issue.

## Delivery strategy
- Working branch: `hdae/getrandom-short-read`
- Intended base: `main`
- Pull request: intended
- Planned commits: signed implementation/gate commit, then an evidence/link reconciliation commit if required
- Actual commits: pending

## Review policy and outcome
Human checkpoint. The gate remains in `gates-todo/` until reviewed and accepted.
