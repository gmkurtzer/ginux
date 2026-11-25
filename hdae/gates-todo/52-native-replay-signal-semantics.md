# Native replay signal semantics

## Source

[Issue #316](https://github.com/gmkurtzer/ginux/issues/316), reporting that a
signal handler can run between private native replay stages, observe rewritten
snapshot registers, and fail the instance when its first syscall does not match
the expected replay entry.

## Goal

Preserve Linux signal and restart semantics across staged clone3, shared-network
send, and inbound SCM_RIGHTS cleanup without exposing private replay registers
to workload signal handlers or making the real operation uninterruptible.

## Approved scope

- Share one save/block/restore primitive for tracee signal masks.
- Block blockable signals only while Ginux transitions into a private staged
  call or cleanup operation.
- Restore the caller mask at the real clone/send entry so the operation remains
  interruptible, then block again before private cleanup and restore the caller
  state at completion.
- Apply the same transition rule to inbound SCM_RIGHTS descriptor cleanup.
- Add deterministic regressions for handler ucontext, `SA_RESTART`/`EINTR`,
  completed sends, clone3 restart, and cleanup interruption.

## Non-goals

- Do not broaden native clone, network, or descriptor authority.
- Do not accept workload syscalls while private replay state is exposed.
- Do not make blocking network operations uninterruptible.
- Do not add general SCM_RIGHTS support; inbound rights remain denied and all
  received descriptors remain closed before the tracee resumes.
- Do not change signal semantics outside staged native replay.

## Completed work

- [x] Maintainer approved the three-phase transition design on 2026-11-25.
- [x] Reconfirmed the defect and existing close-range mask primitive on current
      `main` at `1a72568c`.
- [x] Implemented one shared tracee-mask primitive and explicit begin,
      real-call, private-cleanup, and finish transitions.
- [x] Applied the transition rule to clone3, network send/sendmmsg, Unix and
      netlink sends, child clone cleanup, and inbound SCM_RIGHTS cleanup.
- [x] Restored the complete caller syscall frame before queued signal delivery,
      including all six arguments after inbound descriptor cleanup.
- [x] Added deterministic clone3, nonblocking send, blocking-send
      `EINTR`/`SA_RESTART`, partial-sendmmsg progress, and SCM_RIGHTS cleanup
      regressions.

## Remaining work

- [x] Run focused Linux/root native supervision tests.
- [x] Run `make check`, `make test-unit`, and the practical local suite.
- [x] Commit and push the signed implementation and evidence.
- [x] Add reciprocal issue/PR summaries.
- [ ] Confirm the exact-head `static-and-unit` and `launch-integration` jobs.

## Acceptance criteria

- A handler never observes Ginux snapshot addresses or private map/unmap/close
  register state.
- The real clone/send operation executes with the caller's original signal mask
  and remains interruptible.
- Linux restart behavior is preserved: clone3 restart is transparent;
  interruptible sends follow `SA_RESTART` versus `EINTR`; partial sendmmsg
  progress returns the completed message count.
- Private map, unmap, and SCM_RIGHTS close stages cannot be interposed by a
  workload signal handler.
- Inbound SCM_RIGHTS remains fail-closed: every received descriptor is closed
  while the descriptor-table barrier is held and the caller receives `EPERM`.
- Existing native clone, network snapshot, close-range, descriptor, security,
  and aggregate regressions remain green.

## Verification plan

- A deterministic Linux/root tracer regression queues handled signals at every
  exposed transition and asserts caller-register ucontext plus continued
  instance execution.
- Focused restart cases assert clone3 reissue and network `SA_RESTART`/`EINTR`
  outcomes without private-register exposure.
- SCM_RIGHTS regressions assert all inbound fds become `EBADF`, the caller gets
  `EPERM`, and a queued handler runs only after cleanup.
- `make check` and `make test-unit` -> static and component regressions pass.
- `make test-native-supervision` and the practical aggregate suite -> native
  replay and surrounding runtime behavior pass on Linux/root.
- Fresh exact-head pull-request CI -> required repository jobs pass.

## Evidence

- `make build` -> the freestanding x86-64 Ginux binary and supporting release
  artifacts built successfully on macOS with the repository's `rust-lld`
  flags.
- A fresh disposable Rocky 10 x86-64 QEMU host compiled the native fixture and
  ran `scripts/test-native-replay-signal.py` as root -> all ten clone3, send,
  `EINTR`, `SA_RESTART`, partial-sendmmsg, and inbound-rights transition cases
  passed.
- `make check` -> formatting, strict clippy checks, architecture/name/security
  checks, and generated filesystem-policy documentation checks passed.
- `make test-unit` -> the complete component unit and documentation-test suite
  passed.
- Python no-write compilation plus `git diff --check` -> the new harness parsed
  successfully and the patch has no whitespace errors.
- Exact-head CI at `1f52c201` passed `static-and-unit` but exposed a fast-host
  fixture race: consecutive blocking TCP scenarios reused port 18083 and the
  second bind returned `EADDRINUSE`. The three cases now use distinct allowed
  ports; the complete ten-case Rocky matrix passed again after that correction.

## Decisions and limitations

- The maintainer selected transition-scoped masking. Blocking signals across
  the real operation was rejected because it would make blocking sends
  uninterruptible. Allowing handlers inside replay was rejected because it
  exposes rewritten registers and private mappings. Deferring and re-injecting
  signals was rejected because it must preserve queued real-time signal order
  and siginfo and is unnecessary when the kernel can retain restart ownership.
- The macOS host cannot directly link this freestanding Linux binary with its
  default Apple linker; the authoritative build is `make build`, and native
  behavior was verified in the repository's Rocky x86-64 VM.
- Exact-head pull-request CI remains required for the complete existing native
  supervision matrix, including ordinary sendmmsg completion and surrounding
  clone/network/close-range regressions.
- The failed `1f52c201` integration run was a deterministic fixture-port
  collision, not a replay implementation failure; fresh exact-head CI is still
  required after the isolated-port correction.
- Pull request [#338](https://github.com/gmkurtzer/ginux/pull/338) is open from
  the dedicated Gate 52 branch at implementation commit `a9f39e83`.
- Issue #316 records the delivered fix and remaining review checkpoint in
  [the reciprocal issue summary](https://github.com/gmkurtzer/ginux/issues/316#issuecomment-7171549513);
  [the PR summary](https://github.com/gmkurtzer/ginux/pull/338#issuecomment-7171553719)
  points back to the originating issue and the same evidence boundary.

## Delivery strategy

- Working branch: `hdae/native-replay-signal-semantics`
- Intended base: `main`
- Pull request: [#338](https://github.com/gmkurtzer/ginux/pull/338); it references
  issue #316 without closing it until exact-head review and merge.
- Planned commits: shared replay-mask implementation and regressions; HDAE
  evidence and delivery record.
- Actual commits: `a9f39e83` implements the shared replay-mask behavior and
  deterministic regressions; `893c15ce` records the initial HDAE evidence and
  delivery; `8f133155` records reciprocal delivery links; the final test
  refinement adds deterministic partial-sendmmsg interruption coverage.

## Review policy and outcome

This is a human security and observable-semantics checkpoint. Technical passage
authorizes pull-request delivery, not merge, issue closure, or gate closure.
