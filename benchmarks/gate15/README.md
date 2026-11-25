# Gate 15 benchmark templates

These files are the checked-in inputs for the reproducible dispatch-mode
performance matrix:

- `native.conf` identifies ordinary Rocky execution without Ginux as the
  no-mediation control.
- `selective.conf` identifies the default Ginux syscall-control mode.
- `paranoid.conf` identifies all-syscall Ginux userspace mediation.
- `suite.conf` fixes duration, repetitions, vCPU allocations, workloads, audit
  behavior, and supervisor placement.
- `cpu.conf`, `memory.conf`, `threads.conf`, and `mutex.conf` specify the exact
  sysbench test semantics, direction, unit, thread source, and fixed arguments.

Copy the complete directory before changing a run:

```sh
cp -R benchmarks/gate15 /tmp/my-ginux-benchmark
```

Then pass it to the collector without editing repository scripts:

```sh
GINUX_GATE15_CONFIG_DIR=/tmp/my-ginux-benchmark \
GINUX_GATE15_OUTPUT_DIR=build/my-ginux-benchmark \
    make bench-modes
```

The workload templates deliberately pin the Gate 15 protocol rather than
allowing arbitrary commands to enter the harness. The validator accepts
positive integer durations and repetitions, unique
subsets of vCPUs `1,2,4`, unique subsets of workloads
`cpu,memory,threads,mutex`, boolean `0`/`1` audit controls, and either shared or
an explicit logical CPU `0` through `3` for the supervisor. Profile identity,
runtime, and syscall-control values are fixed so a customized suite cannot
silently relabel its security semantics.

The full checked-in suite uses five-second workloads, five alternating
repetitions, and every 1/2/4-vCPU row. `make demo-gate-15` builds a temporary
bounded suite with one-second CPU and thread tests; it never changes these
templates.
