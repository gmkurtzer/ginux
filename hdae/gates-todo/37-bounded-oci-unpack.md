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

- [ ] Implement pre-allocation output validation and the selected bound.
- [ ] Add regressions and run focused, static, and complete QEMU verification.
- [ ] Push, open the remediation PR, add reciprocal links, and confirm CI.

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
- Actual commits: pending

## Review policy and outcome

The unpack-memory checkpoint was resolved by the human. Implementation and
objective closure may proceed under the user-approved remaining-reports batch;
publishing the PR and reciprocal comments is authorized, but merge and issue
closure are not.
