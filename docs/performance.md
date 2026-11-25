# Reproducible performance measurement

Ginux performance claims are tied to named workloads, checked-in
configurations, raw samples, and a matched Rocky Linux host. Native execution is
the 1.00x control. A result above 1.00x is better and a result below 1.00x is
worse, regardless of whether the underlying metric is throughput or latency.

## Compared execution models

| Report label | Execution | Syscall behavior |
|---|---|---|
| Native | Ordinary Rocky Linux | No Ginux instance or mediation; control only |
| Selective | Ginux native backend | Contract-allowed and independently cataloged direct-safe syscalls may execute directly; all other operations remain Ginux-controlled |
| Paranoid | Ginux native backend | Every syscall enters the Ginux userspace decision path before completion |

Native is sometimes described as the `none` comparison because it has no Ginux
mediation. It is not a supported value for `[contract.syscalls].control` and is
not a Ginux containment mode.

## Fixed protocol

Gate 15 reuses Gate 11's pinned Rocky/EPEL
`sysbench-1.0.20-19.el10_2.x86_64` executable. The host and instance copies must
have the same SHA-256 digest. Every selective and paranoid row is paired with
native samples collected in the same four-vCPU Rocky QEMU host session using:

- the same executable and workload arguments;
- the same 1 GiB address-space limit;
- the same logical-CPU affinity and vCPU allocation;
- one warmup per side and row;
- alternating native/Ginux order by repetition; and
- the same host kernel, image, background environment, and audit setting.

The full suite covers CPU, memory, thread-yield, and mutex workloads. It runs
one thread at 1, 2, and 4 allocated vCPUs, plus allocation-scaled 2-thread and
4-thread rows. The checked-in protocol uses five-second workloads and five
repetitions. Mutex uses the pinned sysbench defaults because a shortened loop
would measure startup rather than sustained mutex behavior.

## Templates

The complete reproducibility inputs live under
[`benchmarks/gate15/`](../benchmarks/gate15/):

- `suite.conf` selects duration, repetitions, allocations, workloads, audit,
  and supervisor placement;
- `native.conf` fixes the control identity;
- `selective.conf` fixes the selective Ginux identity; and
- `paranoid.conf` fixes the paranoid Ginux identity.
- `cpu.conf`, `memory.conf`, `threads.conf`, and `mutex.conf` record every
  workload's exact direction, unit, thread source, duration source, and fixed
  sysbench parameters.

Copy all four files to customize a run. The scripts reject unknown modes,
duplicate or unsupported workload/allocation entries, incomplete profiles, and
incompatible paired metadata before reporting results.

## Running the matrix

Prepare the pinned executable and image once with networking enabled:

```sh
make bench-modes-prepare
```

The preparation cache is retained. Subsequent collection is offline:

```sh
make bench-modes
```

For a quick end-to-end check:

```sh
make demo-gate-15
```

To use copied templates and a separate output directory:

```sh
GINUX_GATE15_CONFIG_DIR=/tmp/my-gate15 \
GINUX_GATE15_OUTPUT_DIR=build/my-gate15 \
    make bench-modes
```

Regenerate the default report without rerunning workloads:

```sh
make bench-modes-report
```

## Outputs

The default output directory is `build/gate15/`:

| Path | Contents |
|---|---|
| `results.tsv` | Combined native/selective/paranoid raw samples with paired-control identity |
| `report.md` | Human-readable means, sample standard deviations, normalized ratios, semantics, and metadata |
| `performance.svg` | One native-relative graph row per workload/allocation/thread combination |
| `config/` | Exact templates copied into the run |
| `selective/` | Selective pair results, metadata, instance recipe, raw transcripts, and bounded audit evidence |
| `paranoid/` | Paranoid pair results, metadata, instance recipe, raw transcripts, and bounded audit evidence |

The validator requires the same commit, tree state, duration, repetitions,
affinity, memory limit, audit setting, host kernel, host resources, executable,
Rocky release, and OCI image across the selective and paranoid pairs. Dispatch
mode and normalized instance-configuration digest are expected to differ.

## Interpretation and limits

Each Ginux value is normalized to the native samples collected beside that
mode, not to a native run from another day. The graph shows performance ratios,
so a higher bar is always better even for lower-is-better mutex latency. Tables
retain means, sample standard deviation, units, and both paired native values;
no aggregate score can conceal an individual row.

Selective and paranoid do not offer identical security behavior. Performance
must be interpreted alongside their syscall-control semantics. The matrix does
not establish universal application performance, production capacity,
multi-instance density, hardware-accelerated behavior, or a new tolerance.
Gate 11 remains authoritative for native-parity goals and exceptions; Gate 13
uses its own current-main regression boundary.

## Troubleshooting

- A missing dependency or image means `make bench-modes-prepare` has not
  completed on the persistent Gate 11 benchmark host.
- A metadata mismatch means the two pairs are not directly comparable. Rerun
  the complete matrix rather than editing metadata.
- A validation failure for missing rows means collection stopped early; retain
  the mode-specific raw directory for diagnosis and rerun the full matrix.
- Generated results remain under ignored `build/`; copy them intentionally when
  sharing evidence, together with all metadata and templates.
