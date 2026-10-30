# Gate 10: Qualification and Comparison

## Goal

Measure the hardened SMP-capable result against the published baseline and
state its remaining limits without overstating the evidence.

## Approved scope

- Run full ABI, lifecycle, Rocky, storage, networking, and multi-domain suites
  with the new two-vCPU default and explicit one/two/four-vCPU coverage.
- Run adversarial tests for the two in-scope security findings.
- Repeat boot, syscall, memory, threading, mutex, storage, and networking
  benchmarks.
- Compare against `research-v1-baseline`.

## Non-goals

- A general kernel security audit.
- Claiming complete Linux compatibility or production security.
- Hiding failed or incomplete gates.

## Completed work

- [x] Freeze the qualification matrix, repetition counts, comparison sources,
  and pass/fail tolerances before running final measurements.

## Remaining work

- Run repeated measurements and publish raw evidence.
- Document residual security, compatibility, hardware, and performance gaps.

## Frozen qualification matrix

The final run uses the following fixed matrix. Functional commands must exit
zero and emit their own exact success marker; a timeout, skipped prerequisite,
external transfer failure, panic, or orphaned QEMU process is not a pass.

| Surface | Command or procedure | Repetition and threshold |
|---|---|---|
| Static, ownership, ABI ledger | `make check` | One clean pass; zero missing Linux ABI rows. |
| Privilege entry and user access | `make architecture-check` and `tools/architecture/reproduce-hardening-feedback.sh` | One pass; all three entry findings resolved/not reproduced, user-copy recovery audit passes, and no tested hostile pointer panics. |
| Deterministic runtime | `make test` and `make test-linux-abi` | One complete pass each with the ordinary two-vCPU default. |
| Topology and stress | `make test-smp` | Full 1/2/4 matrix plus two additional two-vCPU runs. |
| Storage and OCI cache | `make test-storage` and `make test-oci-cache` | Fresh, recovered, reset, corruption, and cache-reuse paths pass. |
| Rocky lifecycle | `make test-rocky-systemd` and `make test-domain-responsiveness` | Attached, warm, detached, login, and controller-latency paths pass. |
| Networking/package workload | `make test-rocky-dnf` | Cached Rocky domain completes the package-network operation. |
| Multiple domains | `make test-multi-domain` | At least two Rocky domains boot and remain operational concurrently on two vCPUs. |
| Published single-CPU comparison | `GINUX_CPUS=1 make benchmark-ginux-boot`, `GINUX_CPUS=1 make benchmark-ginux-sysbench`, and `make benchmark-compare` | Three boot samples and nine samples per sysbench workload. Relative to `docs/paper/artifact/benchmark-2026-10-23`, boot median may increase at most 25%; throughput medians may fall at most 20%; mutex seconds/event may increase at most 20%. |
| SMP scaling | Matched one-thread/one-vCPU and two-thread/two-vCPU CPU and memory sysbench runs using the same local image and binary | Nine samples per workload/topology. Two-vCPU CPU throughput must reach at least 1.50x and memory throughput at least 1.25x the one-vCPU medians. |

Raw final logs and reduced TSV results will be retained under
`docs/feedback-hardening/gate-10-results/`, with the host, QEMU accelerator,
CPU model, memory, image, binary digest, command, and commit recorded. Existing
published results are comparison inputs, not substitutes for fresh hardened
branch measurements.

## Acceptance criteria

- The two reported security defects remain fixed under adversarial regression.
- Invalid tested userspace behavior does not panic the kernel.
- Multiple Rocky domains boot and operate concurrently with two vCPUs.
- One-, two-, and four-vCPU qualification passes.
- Single-CPU compatibility and performance do not materially regress.
- Multi-CPU workloads demonstrate useful scaling.
- Remaining limitations and any failed gates are explicit.

## Evidence

Raw logs, repetition counts, medians/variation, environment description,
baseline comparison, topology matrix, and final limitations register.

## Decisions and limitations

Qualification supports only the tested hardware, workloads, and two explicit
security findings.

## Delivery strategy

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: the shared ten-gate pull request.
- Planned commits: frozen qualification harness, raw/reduced results, and final
  secondary research report.
- Actual commits: pending.

## Review policy and outcome

Delegated technical closure authorized after the full qualification matrix and
baseline comparison pass. Merge and publication remain human decisions.
Outcome: pending.
