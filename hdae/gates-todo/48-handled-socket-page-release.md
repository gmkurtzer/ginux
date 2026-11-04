# Handled socket local-page release

## Source

[GitHub issue #66](https://github.com/gmkurtzer/ginux/issues/66).

## Goal

Release every queued local receive page when a handled TCP or UDP socket closes
so unread fast-path data cannot leak kernel frames.

## Approved scope

- Centralize local-page queue release and state reset.
- Invoke it on local-only, shutdown, and handled-socket close paths.
- Preserve descriptor reference counting and existing peer cleanup behavior.
- Add deterministic multi-page allocator-accounting regression evidence.

## Non-goals

- Adding queue size or per-domain accounting; issue #67 tracks that separately.
- Changing network FIN/RST semantics; issue #68 tracks that separately.
- Reclaiming pages before the final descriptor reference closes.

## Completed work

- [x] Confirmed the handled-socket close branch removes the smoltcp handle and
  clears the slot without releasing `local_first`.

## Remaining work

- [x] Implement shared queue cleanup on every final close path.
- [x] Add deterministic frame-count restoration evidence.
- [x] Run `make check` and `make test`.
- [ ] Push a dedicated branch and open an issue-linked pull request.

## Acceptance criteria

- Final close of a handled socket releases the complete local-page chain before
  the slot is cleared.
- Failed cleanup does not remove the smoltcp handle or erase ownership state.
- Queue head, tail, offset, and byte count reset together after successful
  release.
- Releasing a multi-page queue restores the kernel live-frame count exactly.
- Existing static and complete QEMU regressions pass.

## Verification plan

- Deterministic kernel verifier allocates a multi-page queue, releases it through
  the shared close helper, and proves exact frame-count and state restoration.
- `make check` -> all static, lint, architecture, and host checks pass.
- `make test` -> complete QEMU/Linux runtime suite passes and requires the new
  handled-socket-page-release marker.

## Evidence

- The current handled-socket branch calls `SocketSet::remove` and then assigns
  `Slot::EMPTY` without traversing `local_first`.
- `make check` -> passed all static, lint, architecture, unsafe-inventory, and
  host-side checks.
- `make test` -> passed the complete deterministic QEMU and Linux runtime suite;
  the boot harness required `ginux: handled socket local pages released` and
  ended with `Boot test passed`.

## Decisions and limitations

- Cleanup remains kernel-owner based in this gate. Issue #67 will change queue
  ownership/accounting and must update the same helper coherently.

## Delivery strategy

- Working branch: `hdae/issue-66-socket-page-release`
- Intended base: `main`
- Pull request: intended, dedicated to issue #66
- Planned commits: gate, cleanup helper, and deterministic regression;
  validation and delivery checkpoint
- Actual commits: `b31411fe` (gate, shared cleanup helper, allocator regression,
  and initial evidence); delivery checkpoint pending

## Review policy and outcome

Delegated by the authorized hourly repository-maintenance policy. Merge remains
subject to exact-head CI and the saved two-pass policy.
