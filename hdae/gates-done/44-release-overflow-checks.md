# Gate 44: Enforce Release Overflow Checks

## Source

[GitHub issue #43](https://github.com/gmkurtzer/ginux/issues/43).

## Goal

Make accidental integer overflow fail closed in optimized host, service, boot,
and kernel builds instead of silently wrapping differently from development
builds.

## Approved scope

- Enable Rust overflow checks for the workspace release profile.
- Add a release-mode regression that deliberately performs a runtime size
  overflow and requires a panic.
- Measure EFI image-size and QEMU smoke-test impact before and after the change.
- Run static and complete deterministic QEMU verification.

## Non-goals

- Do not replace intentional wrapping arithmetic such as cryptographic rounds.
- Do not claim overflow checks replace explicit checked arithmetic at untrusted
  parsing boundaries.
- Do not merge the PR or close issue #43.

## Completed work

- [x] Confirmed `[profile.release]` has `panic = "abort"` without
  `overflow-checks`, so optimized arithmetic uses Rust's wrapping default.
- [x] Recorded the baseline EFI size as 70,063,104 bytes and a cached-build
  two-vCPU QEMU smoke wall time of 9.22 seconds.
- [x] Enabled release overflow checks and added a release-only runtime overflow
  regression to the standard static suite.
- [x] Replaced two eager conditional computations whose rejected branches
  underflowed, and derived network address-space validation from the canonical
  161-page checked-release image capacity.
- [x] Raised the generated root publication capacity from 160 to 164 pages for
  the 664,136-byte checked-release root image.
- [x] Measured the post-change image and smoke-test result.
- [x] Ran `make check` and `make test` successfully.

## Remaining work

- None.

## Acceptance criteria

- A release-profile test traps a runtime `usize` size overflow.
- Intentional wrapping arithmetic remains explicit and builds successfully.
- Normal two-vCPU boot reaches the controller and shuts down cleanly.
- Image-size and smoke timing deltas are recorded.
- `make check` and `make test` pass.

## Verification plan

- `cargo test --release --package ginux-oci release_profile_traps_size_overflow`
  -> the `should_panic` regression passes only when release overflow checks are
  active.
- `stat -f '%z' build/esp/EFI/BOOT/BOOTX64.EFI` and
  `/usr/bin/time -p make test-qemu-smoke` -> record post-change size and wall
  time against the baseline.
- `make check` and `make test` -> full static and boot suites pass.

## Evidence

- Issue #43 accurately describes the workspace release profile at
  `main@ecbb3142`.
- Baseline: 70,063,104-byte EFI image; cached-build smoke test passed in 9.22
  seconds wall time.
- `cargo test --release --package ginux-oci
  release_profile_traps_size_overflow` passed and is part of `make check`.
- The first checked-release boot exposed eager rejected-branch underflows in
  MMIO validation and CPU-index validation; both now use lazy `then` branches.
- The checked-release network runtime spans 161 virtual pages. Its loader and
  validator now share the canonical capacity instead of duplicating 160.
- Post-change root image: 664,136 bytes. Post-change EFI: 70,135,296 bytes,
  72,192 bytes (about 0.103%) above baseline.
- Final cached normal-image smoke passed in 9.54 seconds wall time, 0.32 seconds
  above the 9.22-second coarse baseline.
- `make check` passed, including architecture checks, compatibility checks,
  unit tests, Clippy, and the release overflow regression.
- `make test` passed the complete deterministic two-vCPU QEMU verification and
  Linux ELF/runtime compatibility suite.
- Branch `hdae/issue-43-release-overflow` was pushed and published as
  [PR #54](https://github.com/gmkurtzer/ginux/pull/54).
- Issue #43 received the implementation and verification summary with the PR
  link; the issue remains open for maintainer review.
- PR #54 CI passed: `static` in 2m22s and `qemu` in 5m15s.
- After PR #52 merged, PR #54 was synchronized with `main@b4628fc2`; the
  checked-release boot source was regenerated from the merged inputs.
- Fresh `make check` passed on the synchronized tree, including the
  release-profile overflow trap and architecture/compatibility checks.
- Fresh complete `make test` passed on the synchronized tree, including
  `GINUX_KERNEL_OK`, Linux ELF/runtime compatibility, and `Boot test passed`.

## Decisions and limitations

- Overflow checking is a defense-in-depth default. Parsers and allocators must
  continue using checked arithmetic where failure must be recoverable rather
  than aborting the kernel.
- Wall-clock QEMU smoke timing is noisy and is recorded only as a coarse
  regression signal, not a performance benchmark.
- Enabling checks increased optimized code enough to require four additional
  root-image publication pages and one additional network-service image page;
  these remain fixed, bounded capacities.

## Delivery strategy

- Working branch: `hdae/issue-43-release-overflow`
- Intended base: `main`
- Pull request: intended; non-closing reference to issue #43
- Planned commits: gate definition; release profile and regression; evidence;
  delivery
- Actual commits: `a3f0c33d` (gate), `9b11baea` (profile, regression, generated
  image, and capacity), `50e06d76` (checked-release boot invariants),
  `66648d28` (verification evidence), plus the delivery commit and the pending
  signed merge from current `main` containing final synchronized evidence

## Review policy and outcome

Closed under delegated objective completion in the user-approved
remaining-reports batch. All technical criteria and PR CI passed; the branch,
PR, and reciprocal issue summary are published. No merge or issue closure was
performed.
