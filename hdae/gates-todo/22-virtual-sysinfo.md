# Deterministic virtual sysinfo

## Source
[Issue #219](https://github.com/gmkurtzer/ginux/issues/219)

## Goal
Prevent launch/native workloads from observing host uptime, load, memory, swap, and process counts through `sysinfo` while preserving deterministic libc behavior.

## Approved scope
- Emulate successful x86-64 `sysinfo` with a zero-filled structure and `mem_unit = 1`.
- Return `EFAULT` for an unwritable result pointer.
- Cover configured launch and the shared native/systemd supervision loop with
  focused launch/native regressions.

## Non-goals
- Claim or enforce domain memory quotas through `sysinfo`.
- Change process-native behavior or other CPU/topology channels.

## Completed work
- [x] Add the bounded virtual structure and both supervisor paths.
- [x] Add deterministic regression coverage.
- [x] Verify, commit, push, open the PR, and add reciprocal links.

## Remaining work
- [ ] Human review and merge.

## Acceptance criteria
- `sysinfo` returns zero for success, all counters are zero, and `mem_unit` is one.
- A null pointer returns `-EFAULT` without host disclosure.
- Existing checks remain green.

## Verification plan
- `make check` -> formatting, build, clippy, documentation, and static checks pass.
- `sudo make test-launch test-native-supervision` on Linux CI -> launch/native fixtures observe only deterministic virtual values and `EFAULT` for null.

## Evidence
- `make check` -> passed.
- `make test-unit` -> passed across all unit-test crates.
- x86-64 Linux-target compile of both C fixtures with strict warnings -> passed.
- Local launch/native runtime execution -> not run; the macOS arm64 host cannot execute the Linux x86-64 supervisor integration, so hosted CI remains mandatory.
- Pull request: https://github.com/gmkurtzer/ginux/pull/265
- Source comment: https://github.com/gmkurtzer/ginux/issues/219#issuecomment-6841188138
- Reciprocal PR comment: https://github.com/gmkurtzer/ginux/pull/265#issuecomment-6841189329
- Hosted CI on implementation commit `86e6016e`:
  https://github.com/gmkurtzer/ginux/actions/runs/43346029214 -> static/unit
  and Linux launch integration passed.

## Decisions and limitations
- Zero values avoid both host disclosure and a false claim that recipe memory metadata is currently enforced as a quota.
- The systemd instance path shares the native supervisor and therefore receives
  the same virtual structure; the focused Linux CI exercises both branches of
  the emulation helper through configured launch and native mode.
- Process-native and other host-topology channels remain outside this gate.

## Delivery strategy
- Working branch: `hdae/virtual-sysinfo`
- Intended base: `main`
- Pull request: https://github.com/gmkurtzer/ginux/pull/265
- Planned commits: signed implementation/gate commit, then an evidence/link reconciliation commit if required
- Actual commits: `86e6016e`; gate reconciliation in the branch tip

## Review policy and outcome
Technically passed on implementation commit `86e6016e`. Human checkpoint:
the gate remains in `gates-todo/` until reviewed and accepted.
