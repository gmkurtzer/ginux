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
- [x] Run the complete static, architecture, deterministic runtime, ABI,
  storage, OCI, Rocky, package-network, and multi-domain matrix.
- [x] Qualify one, two, and four vCPUs plus two repeated two-vCPU stress runs.
- [x] Repeat the published single-CPU boot and sysbench comparison.
- [x] Run matched one-vCPU/one-thread and two-vCPU/two-thread scaling samples.
- [x] Preserve raw logs, reduced results, failures, environment, and limits.

## Remaining work

- None.

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

- `make check`, `make architecture-check`, and
  `tools/architecture/reproduce-hardening-feedback.sh` passed.
- `make test` and `make test-linux-abi` passed; the ABI report covered 23
  interface groups and 375 syscall identifiers.
- `make test-smp` passed the 1/2/4-vCPU matrix and two repeated 2-vCPU runs.
- `make test-storage`, `make test-oci-cache`, `make test-rocky-systemd`,
  `make test-domain-responsiveness`, `make test-rocky-dnf`, and
  `make test-multi-domain` passed.
- The single-CPU boot median was 14.702 seconds, 14.32% above the published
  12.860-second baseline and within the 25% limit.
- CPU, memory, threads, and mutex sysbench medians changed by -3.94%, -2.74%,
  -11.36%, and +2.39% respectively, all within the 20% limits.
- Two-vCPU CPU scaling reached 2.810x and memory scaling reached 2.352x,
  exceeding the 1.500x and 1.250x requirements.
- Exact-volume `lsof` checks found no QEMU process holding the tested volumes.
- Raw and reduced evidence is under
  `docs/feedback-hardening/gate-10-results/`; the final report is
  `docs/feedback-hardening/GATE-10-QUALIFICATION.md`.

## Decisions and limitations

- Qualification supports only the tested hardware, workloads, ABI surface, and
  two explicit security findings. It is not a general security audit or a
  production-readiness claim.
- Storage and networking were qualified through their frozen functional
  integration tests; the frozen matrix contained no separate numerical
  storage or networking baseline.
- Restricted-host OCI attempts failed before network permission was granted.
  Packet capture showed valid guest requests and host-backend unreachable
  responses; the exact tests then passed with outbound network access.
- The floating upstream Alpine integration image remains a separate follow-up.

## Delivery strategy

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: the shared ten-gate pull request.
- Planned commits: frozen qualification harness, raw/reduced results, and final
  secondary research report.
- Actual commits: `897dd771`, `6d9ef0d5`, `4277fac0`, `0d6ceb9e`, `722c113c`,
  `10192950`, `d3f69f47`, `12e5f935`, `6de84b1c`, `fb8337bb`, `621b2b6f`,
  `8842d4bc`, `e76a2863`, `a4374ec9`, `9f506172`, `112e6b1a`, `f841601f`,
  and `94db6ccc`, followed by the final evidence/closure commit.

## Review policy and outcome

Delegated technical closure authorized after the full qualification matrix and
baseline comparison pass. Merge and publication remain human decisions.
Outcome: passed and closed under the confirmed delegated batch policy.
