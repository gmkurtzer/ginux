# Gate 20: Linux Native-Volume Recovery

## Source

[GitHub issue #14](https://github.com/gmkurtzer/ginux/issues/14): repeated
Linux/x86-64 `make test` runs fault the native volume service and fall back to
tmpfs-only storage across QEMU 10.0.13, 10.1.2, and 11.1.2 under KVM and TCG.

## Goal

Make the native volume service complete and recover consistently during the
full QEMU verification boot on Linux/x86-64 without weakening its IOMMU,
fault-containment, or persistent-storage invariants.

## Approved scope

- Reproduce the report on an isolated Ubuntu x86-64 runner under QEMU TCG.
- Preserve the exact failing serial and QEMU evidence, including the fault
  vector, error, address, instruction, and failed invariant mask.
- Correct the smallest native-service, context, timing, or verification defect
  supported by that evidence.
- Retain a Linux-hosted regression check for the corrected path.

## Non-goals

- Do not hide the fault by accepting tmpfs fallback as a passing storage test.
- Do not weaken IOMMU lifecycle, service isolation, or recovery assertions.
- Do not add retries as a substitute for correcting a deterministic defect.
- Do not close issue #14 or merge the pull request.

## Completed work

- [x] Confirmed the report is distinct from the merged virtio queue-size fix.
- [x] Confirmed current macOS/arm64 QEMU TCG passes while the report spans
  multiple Linux QEMU versions and both KVM and TCG.
- [x] Added a bounded Ubuntu/QEMU TCG reproduction workflow on this branch.

## Remaining work

- [ ] Reproduce and classify the exact Linux failure.
- [ ] Implement the evidence-supported correction and regression coverage.
- [ ] Run `make check` and complete QEMU verification on macOS and Linux.
- [ ] Commit and push the verified correction.
- [ ] Open the remediation pull request and add reciprocal source links.

## Acceptance criteria

- Linux/x86-64 completes the native volume initialization and replacement
  paths without an unexpected ring-3 fault or tmpfs fallback.
- The native-volume IOMMU lifecycle and intentional fault-containment probes
  continue to pass.
- `make check` passes.
- `make test` passes on the available local QEMU environment and on the
  isolated Ubuntu QEMU TCG runner.

## Verification plan

- Branch-only Ubuntu workflow -> reproduces the current failure with full logs
  before the fix, then passes after the correction.
- `make check` -> all static, architecture, lint, and unit checks pass.
- `make test` -> emits `GINUX_KERNEL_OK` and `Boot test passed.` locally.
- GitHub Ubuntu/QEMU TCG job -> emits `GINUX_KERNEL_OK` and completes the boot
  test without native-volume fallback.

## Evidence

- Reporter: same native volume ring-3 #GP at `0x8000005146` across QEMU
  10.0.13, 10.1.2, and 11.1.2, KVM and TCG, one and two vCPUs, rebuilt and
  committed embedded binaries.
- Exact commit `22d20a63` volume-service ELF has executable bytes only through
  relative address `0x4ade`; the reported `0x5146` instruction lies outside
  its executable segment, indicating corrupted or invalid resume state rather
  than a faulting service instruction.
- Linux reproduction run: pending.

## Decisions and limitations

- The local host is macOS/arm64 and cannot validate the reported Linux-host
  behavior directly. The branch-only workflow provides an isolated,
  credential-free Linux reproduction before any remediation pull request is
  opened.
- No pull request will be opened until the branch contains the actual verified
  corrective deliverable.

## Delivery strategy

- Working branch: `hdae/issue-14-native-volume-linux`
- Intended base: `main`
- Pull request: intended after the verified correction exists; non-closing
  reference to issue #14
- Planned commits: reproduction evidence; correction and regression; delivery
  checkpoint
- Actual commits: pending

## Review policy and outcome

Delegated by the saved proactive scheduled-review policy. Keep the gate in
`gates-todo/`; successful delivery requires a verified fix, pushed branch,
open pull request, and reciprocal links.
