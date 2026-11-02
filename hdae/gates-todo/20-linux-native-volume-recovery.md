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
- [x] Ran the first isolated Linux reproduction and posted its evidence and
  focused log request to the source issue.
- [x] Classified the PR #36 and #37 failures as normal recovery generations
  reaching `ScanError::Device` after an earlier fault or forced cancellation;
  teardown completed and the timeout counter did not advance.

## Remaining work

- [x] Reproduce and classify the reported ring-3 failure path.
- [x] Implement the evidence-supported correction using the existing recovery
  sequence as its integration regression.
- [x] Run `make check` and complete QEMU verification on macOS and Linux.
- [x] Commit and push the verified correction.
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
- [Ubuntu reproduction run 41334525169](https://github.com/gmkurtzer/ginux/actions/runs/41334525169)
  built successfully under QEMU 8.2.2 but timed out before the controller
  prompt because that QEMU defaulted to an insufficient IOMMU address width;
  it did not reproduce the reported `0x5146` fault.
- [Issue #14 response](https://github.com/gmkurtzer/ginux/issues/14#issuecomment-6562031525)
  records the result and requests one complete serial and QEMU interrupt log.
- PR #36 failed after cancellation at service offset `0x1c46`; PR #37 failed
  after the intentional fault at the same offset. Release disassembly maps
  that instruction to `Err(ScanError::Device) => fail()` in the normal volume
  recovery path. Both runs retained zero auxiliary roots and frames, and
  neither advanced the timeout count.
- A masked request from the fault or cancellation generation can complete
  after teardown begins, leaving MSI-X PBA vector 0 pending. The next fresh
  generation unmasks that stale event before its new request completes and
  consequently observes an incomplete used ring.
- `make check` -> passed, including the unchanged 1,033-block kernel unsafe
  baseline and all architecture, lint, unit, and ABI checks.
- Three consecutive local `make test` runs on QEMU 10.1.2/TCG -> passed;
  every run completed the intentional fault and forced-cancellation recovery
  paths, emitted `GINUX_KERNEL_OK`, and reported `Boot test passed.`
- [Ubuntu correction run 41367841412](https://github.com/gmkurtzer/ginux/actions/runs/41367841412)
  built commit `17284060` successfully but again timed out after 300 seconds
  before emitting any serial output. This is an earlier Linux/OVMF boot stall,
  not evidence that the corrected native-volume recovery path failed.
- [Ubuntu diagnostic run 41368554650](https://github.com/gmkurtzer/ginux/actions/runs/41368554650)
  preserved four empty boot logs and no QEMU debug log. That proves the
  launcher exited before QEMU startup: Ubuntu 24.04 provides only the 4 MiB
  OVMF filenames, which `common.sh` did not search. The harness also wrote its
  PID file after firmware discovery, so the producer could not detect that
  early launcher exit and waited for the full timeout.
- [Ubuntu verification run 41369349235](https://github.com/gmkurtzer/ginux/actions/runs/41369349235)
  passed the complete `make test` boot on Ubuntu/QEMU TCG after the stale
  interrupt drain and launcher corrections.

## Decisions and limitations

- The local host is macOS/arm64 and cannot validate the reported Linux-host
  behavior directly. The branch-only workflow provides an isolated,
  credential-free Linux reproduction before any remediation pull request is
  opened.
- No pull request will be opened until the branch contains the actual verified
  corrective deliverable.
- PR #31 independently corrects QEMU 8.2's launcher address-width mismatch;
  Gate 20 still needs evidence from the reported QEMU 10/11 environment rather
  than conflating that separate compatibility problem with the service fault.
- The correction drains a detected stale PBA event while no generation is
  active, then resets the software interrupt count before publishing the fresh
  domain and request. It does not accept an incomplete request or weaken any
  teardown assertion.
- The branch-only workflow now preserves all boot logs plus QEMU interrupt and
  guest-error diagnostics as an artifact on both success and failure; ordinary
  local and repository test behavior is unchanged unless the artifact
  directory environment variable is set.
- The launcher now recognizes Ubuntu's `OVMF_CODE_4M.fd` and
  `OVMF_VARS_4M.fd` pair and publishes its PID before dependency discovery, so
  missing prerequisites fail promptly instead of appearing as a QEMU hang.

## Delivery strategy

- Working branch: `hdae/issue-14-native-volume-linux`
- Intended base: `main`
- Pull request: intended after the verified correction exists; non-closing
  reference to issue #14
- Planned commits: reproduction evidence; correction and regression; delivery
  checkpoint
- Actual commits: `1358dd1c` reproduction workflow and checkpoint; `17284060`
  stale PBA drain correction; `19e68576` preserved Linux boot diagnostics;
  `04d377a5` Ubuntu 4 MiB OVMF discovery and prompt launcher-failure detection

## Review policy and outcome

Delegated by the saved proactive scheduled-review policy. Keep the gate in
`gates-todo/`; successful delivery requires a verified fix, pushed branch,
open pull request, and reciprocal links.
