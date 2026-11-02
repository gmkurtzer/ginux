# Gate 32: TLS Client Entropy

## Source

[GitHub issue #15](https://github.com/gmkurtzer/ginux/issues/15).

## Goal

Give every registry TLS connection a fresh cryptographic seed that cannot be
reconstructed from the RTC boot second, and fail closed when hardware entropy
is unavailable.

## Approved scope

- Replace the RTC-only TLS seed with four independently successful 64-bit
  `RDRAND` samples per connection.
- Verify CPU support before executing `RDRAND` and bound retries for each word.
- Abort the TLS connection before key generation when entropy cannot be
  obtained.
- Retain the boot time only for network timekeeping, never as TLS key material.

## Non-goals

- Do not redesign the native service ABI or build a general kernel CSPRNG.
- Do not weaken certificate or hostname verification.
- Do not close issue #15 or merge the pull request.

## Completed work

- [x] Confirmed `seed()` deterministically expands one boot-second value and is
  called once for every TLS connection.
- [x] Confirmed the target x86-64 profile and kernel already use bounded
  CPUID-gated `RDRAND` sampling.
- [x] Replaced the RTC-derived seed with four fresh bounded hardware samples
  per connection and fail-closed handling before TLS setup.
- [x] Corrected the inline-assembly flag contract so the compiler is informed
  that `RDRAND` changes condition flags.
- [x] Passed targeted lint, full static checks, and the complete QEMU boot.

## Remaining work

- [x] Implement fail-closed per-connection hardware seeding.
- [x] Run static and QEMU verification.
- [x] Commit and push the dedicated branch.
- [x] Open the remediation pull request and add reciprocal source links.

## Acceptance criteria

- No TLS seed is derived solely or predictably from the RTC boot second.
- Every TLS connection obtains a fresh 256-bit seed from successful hardware
  random samples.
- Missing CPU support or exhausted retries aborts TLS setup before key use.
- Existing certificate, hostname, build, and QEMU verification pass.

## Verification plan

- Source inspection and targeted search -> no RTC-derived TLS seed remains and
  every provider construction handles entropy failure.
- `make check` -> formatting, architecture, lint, and unit checks pass.
- `make test` -> complete QEMU verification emits `GINUX_KERNEL_OK` and
  `Boot test passed.`

## Evidence

- `services/network/src/registry.rs::seed` repeats the boot `unix_time` into
  four predictable words, and `https_get` constructs a new provider from that
  same value for every request.
- `cargo fmt --all -- --check` and targeted warning-denying network-service
  clippy -> passed.
- `make check` -> passed all static, architecture, lint, and unit checks.
- `make test` on QEMU 10.1.2/TCG -> emitted `GINUX_KERNEL_OK` and
  `Boot test passed.`
- Maintainer review found and removed the false `preserves_flags` declaration
  from the `RDRAND`/`setc` assembly; targeted clippy, fresh `make check`, and
  fresh `make test` all passed afterward.

## Decisions and limitations

- TLS fails closed without `RDRAND`; predictable fallback key material is not
  acceptable.
- This gate fixes the reported TLS seed defect but does not claim a general
  entropy subsystem for all kernel and userspace consumers.

## Delivery strategy

- Working branch: `hdae/issue-15-tls-entropy`
- Intended base: `main`
- Pull request: [#40](https://github.com/gmkurtzer/ginux/pull/40); non-closing
  reference to issue #15
- Planned commits: implementation and regression evidence; delivery checkpoint
- Actual commits: `62756f4d` (implementation, generated boot image, and
  verification evidence), `4d12e47d` (delivery checkpoint), assembly contract
  correction and review evidence pending

## Review policy and outcome

Delegated by the saved proactive scheduled-review policy. Keep this gate in
`gates-todo/`; successful delivery requires a verified fix, pushed branch,
open pull request, and reciprocal links.
