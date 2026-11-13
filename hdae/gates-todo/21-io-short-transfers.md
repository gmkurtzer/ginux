# Linux-compatible bounded I/O short transfers

## Source
[Issue #178](https://github.com/gmkurtzer/ginux/issues/178)

## Goal
Preserve balanced-profile read, write, and pread bounds while returning Linux-compatible short transfers instead of `EPERM` for oversized requests.

## Approved scope
- Clamp balanced `read` and `pread64` lengths to 1 MiB.
- Clamp balanced stdout/stderr `write` lengths to 4096 bytes.
- Add focused unit and launch/native regressions.

## Non-goals
- Change systemd-instance I/O limits.
- Add aggregate-byte accounting to `writev`; that remains a separate contract decision.

## Completed work
- [x] Implement balanced-path register clamping.
- [x] Add deterministic regression coverage.
- [x] Verify, commit, push, open the PR, and add reciprocal links.

## Remaining work
- [ ] Human review and merge.

## Acceptance criteria
- Oversized balanced reads and preads return exactly 1 MiB when sufficient input exists.
- An oversized balanced stdout write returns 4096 and emits only 4096 bytes.
- Systemd-mode limits are unchanged and existing checks remain green.

## Verification plan
- `cargo test -p ginux-core` -> balanced policy bounds remain enforced.
- `make check` -> formatting, build, clippy, documentation, and static checks pass.
- `sudo make test-launch test-native-supervision` on Linux CI -> real launch/native paths demonstrate short transfers.

## Evidence
- `cargo test -p ginux-core` -> 9 passed.
- `make test-unit` -> passed across all unit-test crates.
- `make check` -> passed.
- x86-64 Linux-target compile of both C fixtures with strict warnings -> passed.
- Local launch/native runtime execution -> not run; the macOS arm64 host cannot execute the Linux x86-64 supervisor integration, so hosted CI remains mandatory.
- Pull request: https://github.com/gmkurtzer/ginux/pull/264
- Source comment: https://github.com/gmkurtzer/ginux/issues/178#issuecomment-6841185350
- Reciprocal PR comment: https://github.com/gmkurtzer/ginux/pull/264#issuecomment-6841187126

## Decisions and limitations
- Register clamping preserves the resource ceiling and the Linux short-transfer contract.
- `writev` remains out of scope because its iovec aggregation is independently reviewable.

## Delivery strategy
- Working branch: `hdae/io-short-transfers`
- Intended base: `main`
- Pull request: https://github.com/gmkurtzer/ginux/pull/264
- Planned commits: signed implementation/gate commit, then an evidence/link reconciliation commit if required
- Actual commits: `63655990`; gate reconciliation in the branch tip

## Review policy and outcome
Human checkpoint. The gate remains in `gates-todo/` until reviewed and accepted.
