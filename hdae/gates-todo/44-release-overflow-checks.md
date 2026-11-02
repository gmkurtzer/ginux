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

## Remaining work

- [ ] Enable checked release arithmetic and add the release regression.
- [ ] Measure the post-change image and smoke-test result.
- [ ] Run `make check` and `make test`.
- [ ] Push, open the remediation PR, add reciprocal links, and confirm CI.

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

## Decisions and limitations

- Overflow checking is a defense-in-depth default. Parsers and allocators must
  continue using checked arithmetic where failure must be recoverable rather
  than aborting the kernel.
- Wall-clock QEMU smoke timing is noisy and is recorded only as a coarse
  regression signal, not a performance benchmark.

## Delivery strategy

- Working branch: `hdae/issue-43-release-overflow`
- Intended base: `main`
- Pull request: intended; non-closing reference to issue #43
- Planned commits: gate definition; release profile and regression; evidence;
  delivery
- Actual commits: pending

## Review policy and outcome

Delegated objective completion under the user-approved remaining-reports batch.
Publishing the PR and reciprocal comments is authorized; merge and issue
closure are not.
