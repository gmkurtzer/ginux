# Gate 37: Bounded OCI Layer Unpack

## Source

[GitHub issue #20](https://github.com/gmkurtzer/ginux/issues/20).

## Goal

Reject attacker-controlled gzip expansion sizes before ring-0 allocation and
zeroing, while preserving valid OCI layer semantics and supported images.

## Approved scope

- Reproduce the forged gzip `ISIZE` allocation request.
- Validate gzip output sizing before allocating rootfs backing.
- Enforce the human-selected unpack-memory policy before allocation.
- Fail image start without publishing a partial rootfs or destabilizing other
  domains.
- Add forged-size, expansion-limit, cleanup, and valid-layer regressions.

## Non-goals

- Do not compare gzip output size with OCI descriptor `size`; it is compressed
  blob length.
- Do not reintroduce a fixed persistent compressed-layer cache quota.
- Do not merge the PR or close issue #20.

## Completed work

- [x] Confirmed `install_published_layers` trusts gzip `ISIZE` before allocating
  and zeroing the combined rootfs extent.
- [x] Confirmed OCI manifest layer size cannot validate uncompressed length.
- [x] Confirmed no existing per-layer or assembled-rootfs RAM limit exists.
- [x] Human selected a separate host-wide configurable rootfs-unpack RAM
  policy with a 512-MiB default.

## Remaining work

- [x] Implemented pre-allocation output validation and the selected bound.
- [x] Added regressions and ran focused, static, and complete QEMU verification.
- [x] Pushed, opened PR #52, and added the reciprocal issue link. GitHub CI
  remains external follow-up evidence and does not replace the fresh local
  acceptance suite.

## Acceptance criteria

- A tiny layer with a forged near-4-GiB `ISIZE` is rejected without requesting
  or zeroing that extent.
- A valid expansion beyond the selected limit is rejected before publication
  and leaves staging/rootfs resources reclaimable.
- Valid layers within the limit still install.
- Static and complete QEMU verification pass.

## Verification plan

- Image-store regression -> forged `ISIZE` causes no large allocation or
  rootfs publication.
- Expansion-bound regression -> output beyond policy fails with cleanup.
- Existing OCI/rootfs tests -> valid parsing, whiteouts, and publication pass.
- `make check` and `make test` -> full static and boot checks pass.

## Evidence

- Issue #20 matches `kernel/src/image_store.rs`.
- `ARCHITECTURE.md` requires unpack resource exhaustion to fail the initiating
  operation without destabilizing other domains, but defines no RAM limit.
- `make check` -> passed, including architecture, compatibility, clippy, and
  all host-side crate tests.
- `make test` -> passed the complete multi-boot QEMU sequence and emitted
  `ginux: bounded OCI rootfs unpack policy verified`.
- PR #52: <https://github.com/gmkurtzer/ginux/pull/52>.
- Issue summary: <https://github.com/gmkurtzer/ginux/issues/20#issuecomment-6569098566>.

## Decisions and limitations

- The human selected a separate host-wide configurable rootfs-unpack RAM
  policy with a 512-MiB default. The install path receives the policy
  explicitly rather than conflating immutable-rootfs staging with a domain's
  private `memory_limit_mib`.

## Delivery strategy

- Working branch: `hdae/issue-20-gzip-unpack`
- Intended base: `main`
- Pull request: intended; non-closing reference to issue #20
- Planned commits: gate definition; bounded preflight/allocator mechanism;
  regressions/evidence; delivery
- Actual commits: `1bce59ce` defines the gate; `8667f694` implements the
  policy, regressions, and fresh evidence integration; final delivery record
  follows in the closing commit.

## Review policy and outcome

The unpack-memory checkpoint was resolved by the human. Gate 37 passed every
objective criterion and closed under the user-approved remaining-reports batch.
PR #52 and the reciprocal issue summary were published; merge and issue closure
remain maintainer actions.
