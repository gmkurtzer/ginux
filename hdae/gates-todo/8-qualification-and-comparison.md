# Gate 8: Qualification and Comparison

## Goal

Measure the hardened result against the published baseline and state its
remaining limits without overstating the evidence.

## Approved scope

- Run full ABI, lifecycle, Rocky, storage, networking, and multi-domain suites
  on one, two, and four CPUs.
- Run adversarial privilege-entry and userspace-pointer suites.
- Repeat boot, syscall, memory, threading, mutex, storage, and networking
  benchmarks.
- Compare against `research-v1-baseline`.

## Non-goals

- Claiming complete Linux compatibility or production security.
- Hiding failed or incomplete gates.

## Completed work

None. Depends on Gate 7.

## Remaining work

- Freeze the qualification matrix and tolerances.
- Run repeated measurements and publish raw evidence.
- Document residual security, compatibility, hardware, and performance gaps.

## Acceptance criteria

- Invalid userspace behavior does not panic the kernel in the tested matrix.
- Multiple Rocky domains boot and operate concurrently.
- Single-CPU compatibility and performance do not materially regress.
- Multi-CPU workloads demonstrate useful scaling.
- Remaining limitations and any failed gates are explicit.

## Evidence

Raw logs, repetition counts, medians/variation, environment description,
baseline comparison, and final limitations register.

## Decisions and limitations

Qualification supports only the tested hardware, workloads, and threat cases.

## Delivery strategy

Planned commits: frozen qualification harness, raw/reduced results, and final
secondary research report.

## Review policy and outcome

Checkpoint required. Stop for publication decision. Outcome: pending.
