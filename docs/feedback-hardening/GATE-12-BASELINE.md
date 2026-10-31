# Gate 12 Baseline

Gate 12 began from merged `main` commit `a2a1eace`. The matched matrix used
the pinned x86-64 sysbench binary, Q35 under TCG, 2 GiB memory, nine samples
per workload cell, and identical vCPU/thread topology for Rocky-on-Linux and
Rocky-on-Ginux.

The table reports the favorable-direction Ginux/Linux ratio. Values below the
gate floor of `0.96x` are marked **fail**.

| Mode | vCPUs | Threads | CPU | Memory | Thread yield | Mutex |
|---|---:|---:|---:|---:|---:|---:|
| fixed | 1 | 1 | 1.216x | 1.169x | 0.994x | 1.079x |
| fixed | 2 | 1 | 1.075x | 1.140x | **0.346x fail** | 1.079x |
| fixed | 4 | 1 | 1.039x | 1.096x | **0.323x fail** | 1.050x |
| scaled | 2 | 2 | 1.228x | 1.044x | 1.123x | **0.810x fail** |
| scaled | 4 | 4 | **0.595x fail** | **0.668x fail** | **0.014x fail** | **0.389x fail** |

The fixed 2-vCPU thread-yield profile recorded about 1.27 million syscalls per
four-second sample: roughly half `futex` and half `sched_yield`. The main CPU
returned a successful spurious futex wake while the single remote worker ran,
causing userspace to re-enter the kernel continuously. The 4-vCPU result also
matches the implementation limit of one remote runtime-thread job pinned to
CPU 1; CPUs 2 and 3 cannot execute Linux runtime threads.

Raw generated results remain under `build/benchmark/gate12-baseline/` in the
working tree and are intentionally not committed.
