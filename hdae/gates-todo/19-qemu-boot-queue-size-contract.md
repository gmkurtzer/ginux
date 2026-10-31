# Gate 19: QEMU Boot Queue-Size Contract

## Source

[GitHub issue #4](https://github.com/gmkurtzer/ginux/issues/4): after PR #10
made the kernel accept supported virtio-net queue sizes, QEMU 10.0.13 still
causes `make test` to fail because the host-side boot verifier requires exactly
256 entries.

## Goal

Make the QEMU boot verifier accept the same supported virtio-net queue-size
contract as the kernel so QEMU 10.0.13's 1024-entry queues pass while invalid
sizes remain rejected.

## Approved scope

- Replace the two fixed 256-entry serial-log checks with the complete supported
  power-of-two range from 8 through 32768.
- Accept serial output with or without a trailing carriage return.
- Run static checks and the complete QEMU verification boot.

## Non-goals

- Do not change virtio queue allocation, negotiation, or kernel validation.
- Do not add a QEMU version matrix or pin a host QEMU version.
- Do not close issue #4 or merge the pull request.

## Completed work

- [x] Reproduced the contract mismatch by source inspection against merged
  PR #10 and the contributor's QEMU 10.0.13 evidence.
- [x] Updated both receive and transmit serial-log checks to accept every
  kernel-supported queue size.

## Remaining work

- [x] Run deterministic static and QEMU verification.
- [ ] Commit and push the dedicated branch.
- [ ] Open the remediation pull request and add reciprocal links.

## Acceptance criteria

- The host verifier accepts receive and transmit queue sizes 256 and 1024.
- The host verifier rejects 0, 7, 255, 1000, and values above the virtio `u16`
  power-of-two maximum.
- Serial lines with a trailing carriage return are accepted.
- `make check` and `make test` pass.

## Verification plan

- Feed representative valid, invalid, and CR-terminated lines to the exact
  regular expression -> 256 and 1024 pass; invalid sizes fail.
- `make check` -> all static, architecture, lint, and unit checks pass.
- `make test` -> the complete QEMU verification boot emits `GINUX_KERNEL_OK`
  and `Boot test passed.`

## Evidence

- Issue #4 contributor evidence: PR #10's kernel fix boots on QEMU 10.0.13,
  then `tests/qemu/boot.sh` rejects the emitted 1024 queue size.
- Exact regular-expression probes -> accepted 256, 1024, and a CR-terminated
  1024 line; rejected 0, 7, 255, 1000, and 65536.
- `make check` -> passed all builds, static checks, architecture checks,
  warning-denying lints, and unit tests.
- `make test` on QEMU 10.1.2 under TCG -> emitted `GINUX_KERNEL_OK` and
  `Boot test passed.`

## Decisions and limitations

- The verifier enumerates the finite `u16` powers of two accepted by the
  kernel. This keeps the host check explicit without adding another parser.
- QEMU 10.0.13 is not installed in the scheduled runner. The contributor's
  three-run evidence covers that exact host version; this branch will run the
  full QEMU suite on the available host version.

## Delivery strategy

- Working branch: `hdae/issue-4-boot-queue-size`
- Intended base: `main`
- Pull request: intended; non-closing reference to issue #4
- Planned commits: one signed implementation, gate, and verification commit
- Actual commits: pending

## Review policy and outcome

Delegated by the saved proactive scheduled-review policy. Keep this gate in
`gates-todo/` for maintainer review after the verified branch, open pull
request, and reciprocal source links exist.
