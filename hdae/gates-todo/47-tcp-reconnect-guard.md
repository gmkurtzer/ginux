# TCP reconnect guard

## Source

[GitHub issue #69](https://github.com/gmkurtzer/ginux/issues/69).

## Goal

Reject a second `connect()` on a connected local fast-path TCP socket before it
can enqueue a duplicate accept or repoint the peer.

## Approved scope

- Add an explicit connected-socket result to the kernel socket contract.
- Apply the guard before local-port allocation or listener mutation.
- Preserve the existing in-progress behavior for network TCP connects.
- Add deterministic regression evidence for same- and different-endpoint
  reconnect attempts sharing the same connected state.

## Non-goals

- Changing TCP backlog or listener lifetime.
- Changing UDP reconnect behavior.
- Reworking accepted-socket ownership.

## Completed work

- [x] Confirmed the fast-path branch runs before the smoltcp active-state guard
  and appends the same connector to the pending queue again.

## Remaining work

- [x] Implement the fail-before-mutation reconnect guard and Linux `EISCONN`.
- [x] Add deterministic regression evidence.
- [x] Run `make check` and `make test`.
- [ ] Push a dedicated branch and open an issue-linked pull request.

## Acceptance criteria

- A connected fast-path TCP slot returns `EISCONN` for any second endpoint.
- No listener pending length, peer identity, or local endpoint changes on the
  rejected call.
- Initial TCP and UDP connects retain their current behavior.
- Existing static and complete QEMU regressions pass.

## Verification plan

- Deterministic reconnect-state verifier -> connected TCP is rejected while
  fresh TCP and UDP states are not.
- `make check` -> all static, lint, architecture, and host checks pass.
- `make test` -> complete deterministic QEMU and Linux runtime suite passes and
  requires the reconnect-guard marker.

## Evidence

- `kernel/src/linux_socket.rs::connect` mutates the fast-path listener queue and
  returns before its later smoltcp active-state check.
- `make check` -> passed every static, lint, architecture, unsafe-inventory,
  and host-side check.
- `make test` -> passed the complete QEMU and Linux runtime suite; the boot
  harness required `ginux: TCP reconnect guard verified` and ended with `Boot
  test passed`.

## Decisions and limitations

- `peer.is_some()` is the authoritative local fast-path connected state before
  and after accept; the guard therefore precedes every fallible or visible
  mutation.

## Delivery strategy

- Working branch: `hdae/issue-69-reconnect-guard`
- Intended base: `main`
- Pull request: intended, dedicated to issue #69
- Planned commits: gate, implementation, and regression; validation and
  delivery checkpoint
- Actual commits: `9f96395e` (gate, reconnect guard, Linux errno mapping,
  regression, and initial evidence); delivery checkpoint pending

## Review policy and outcome

Delegated by the authorized hourly repository-maintenance policy. Merge remains
subject to exact-head CI and the saved two-pass policy.
