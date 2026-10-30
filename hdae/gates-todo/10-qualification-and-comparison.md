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

None. Depends on Gate 9.

## Remaining work

- Freeze the qualification matrix and tolerances.
- Run repeated measurements and publish raw evidence.
- Document residual security, compatibility, hardware, and performance gaps.

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

Checkpoint required. Stop for publication and merge decision. Outcome: pending.
