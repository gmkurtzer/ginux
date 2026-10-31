# Gate 12 Performance Parity

Gate 12 compared the same pinned x86-64 sysbench binary on Rocky Linux under
Linux and Rocky Linux userspace under Ginux. Both sides used Q35 under QEMU
TCG, 2 GiB memory, identical one-, two-, and four-vCPU topologies, and nine
samples per workload cell. Ratios below use the favorable direction: higher
throughput is better for CPU, memory, and thread yield; lower elapsed time is
better for mutex.

## Result

Every cell meets the approved `0.96x` floor. Eighteen of the twenty unique
cells beat Linux. Fixed 2-vCPU CPU throughput reached `0.982x`, and scaled
4-vCPU mutex reached `0.970x`; both are within the accepted two-to-four-percent
band but did not reach the stronger faster-than-Linux target.

| Mode | vCPUs | Threads | CPU | Memory | Thread yield | Mutex |
|---|---:|---:|---:|---:|---:|---:|
| fixed | 1 | 1 | 2.185x | 2.342x | 1.312x | 2.163x |
| fixed | 2 | 1 | 0.982x | 1.122x | 2.995x | 1.066x |
| fixed | 4 | 1 | 1.002x | 1.101x | 2.912x | 1.036x |
| scaled | 2 | 2 | 1.186x | 1.207x | 4.090x | 1.070x |
| scaled | 4 | 4 | 1.173x | 1.138x | 2.583x | 0.970x |

## Before and after

The merged Gate 11 baseline had seven failing cells. The largest gaps were
scaled 4-vCPU thread yield at `0.014x`, mutex at `0.389x`, CPU at `0.595x`,
and memory at `0.668x`. Gate 12 changed general runtime mechanisms rather than
recognizing or special-casing sysbench:

- a remote `sched_yield` fast path stopped needlessly serializing a thread
  that could only select itself;
- Linux runtime jobs and ownership publication became per-CPU, allowing all
  online application CPUs to execute independent Linux threads;
- futex wait publication and wakeup gained a coordinated boundary;
- safe fast syscalls remain available while an interval timer is armed but no
  signal is pending; and
- Linux `FUTEX_WAIT_BITSET` and `FUTEX_WAKE_BITSET` masks, absolute timeout
  semantics, and sixth-argument wiring were implemented. A guarded remote-
  worker path removed a userspace retry storm caused by rejecting valid glibc
  futex operations.

Profiling found roughly 699,000 operation-265 observations in one pre-fix
scaled memory sample. Operation 265 is
`FUTEX_WAIT_BITSET|FUTEX_CLOCK_REALTIME`; Ginux had returned `EINVAL`, causing
glibc to re-enter the kernel continuously. After correction, focused scaled
memory and mutex medians crossed the acceptance floor, and the final complete
matrix confirmed the result.

## Qualification

The final code passed:

- `make check`
- `make architecture-check`
- `make test-smp`
- `make test-rocky-smp-futex`
- `make test-rocky-smp-shared-page`
- `make test-rocky-smp-thread-context`

The final matrix contains nine Linux and nine Ginux samples in every sysbench
cell and uses the same binary digest throughout. Boot measurements remain
observational because Linux and Ginux follow different startup paths. These
results characterize this QEMU TCG experiment; they are not native-hardware or
general production-performance claims.
