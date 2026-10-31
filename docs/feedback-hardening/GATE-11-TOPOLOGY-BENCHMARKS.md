# Gate 11 Matched Topology Benchmarks

Gate 11 measures the same Rocky Linux 10 userspace and pinned x86-64 sysbench
binary on Linux and Ginux under matched one-, two-, and four-vCPU QEMU TCG
configurations. It separates a fixed one-thread series from a scaled series in
which workload threads equal the vCPU count.

## Environment and controls

- Ginux commit: `d402eee55dc11aac3b2a77a88d897bacd3f65dc3`
- Host: Apple Silicon, macOS 26.6.2 (25G83)
- Emulator: QEMU 10.1.2, x86-64 TCG
- Machine and CPU: Q35, `max,+smep,+smap,+umip`
- Guest memory: 2 GiB
- Sysbench binary SHA-256:
  `7cf6b71521436bd32a8ab197851035ff1632bee383fec5a35468d61969e7c18d`
- Samples: nine per sysbench cell and three per boot cell after warm-up

The summarizer rejects missing samples, mismatched Linux/Ginux binary hashes,
multiple hashes across cells, and unexpected topology metadata. Every check
passed. QEMU children were reaped and the benchmark volume had no remaining
owner.

## Fixed one-thread results

The ratio favors Ginux: throughput uses Ginux/Linux, while mutex time uses
Linux/Ginux because lower time is better.

| vCPUs | Workload | Linux | Ginux | Ginux/Linux |
|---:|---|---:|---:|---:|
| 1 | CPU events/s | 229.030 | 273.090 | 1.192x |
| 1 | Memory MiB/s | 670.400 | 800.260 | 1.194x |
| 1 | Threads events/s | 292.158 | 285.486 | 0.977x |
| 1 | Mutex s/event | 1.568 | 1.420 | 1.104x |
| 2 | CPU events/s | 501.490 | 504.590 | 1.006x |
| 2 | Memory MiB/s | 1385.420 | 1538.340 | 1.110x |
| 2 | Threads events/s | 673.042 | 209.555 | 0.311x |
| 2 | Mutex s/event | 0.763 | 0.723 | 1.055x |
| 4 | CPU events/s | 506.070 | 520.560 | 1.029x |
| 4 | Memory MiB/s | 1393.290 | 1562.250 | 1.121x |
| 4 | Threads events/s | 687.369 | 222.474 | 0.324x |
| 4 | Mutex s/event | 0.756 | 0.709 | 1.066x |

## Scaled-thread results

| vCPUs/threads | Workload | Linux | Ginux | Ginux/Linux |
|---:|---|---:|---:|---:|
| 1/1 | CPU events/s | 229.030 | 273.090 | 1.192x |
| 1/1 | Memory MiB/s | 670.400 | 800.260 | 1.194x |
| 1/1 | Threads events/s | 292.158 | 285.486 | 0.977x |
| 1/1 | Mutex s/event | 1.568 | 1.420 | 1.104x |
| 2/2 | CPU events/s | 634.430 | 811.110 | 1.278x |
| 2/2 | Memory MiB/s | 1456.360 | 1504.170 | 1.033x |
| 2/2 | Threads events/s | 441.456 | 380.949 | 0.863x |
| 2/2 | Mutex s/event | 1.132 | 1.449 | 0.781x |
| 4/4 | CPU events/s | 1354.130 | 891.670 | 0.658x |
| 4/4 | Memory MiB/s | 1966.910 | 1649.720 | 0.839x |
| 4/4 | Threads events/s | 1016.582 | 85.929 | 0.085x |
| 4/4 | Mutex s/event | 1.214 | 2.857 | 0.425x |

Relative to each system's own one-vCPU result, Ginux reaches 3.265x CPU and
2.061x memory throughput at four vCPUs/four threads. Linux reaches 5.912x and
2.934x respectively. Ginux thread-yield throughput falls to 0.301x its
one-vCPU result at four threads, while Linux reaches 3.480x. Ginux mutex
performance also falls to 0.497x its baseline, versus Linux at 1.292x.

The fixed CPU, memory, and mutex results are close in this translated test
environment, and the two-thread scaled results remain within a narrower range.
The four-thread series exposes a substantial Ginux scheduling and yield
scalability limitation, especially in the threads workload. Gate 11 records
that limitation; performance tuning was explicitly outside its measurement
scope.

## Boot observations

| vCPUs | Rocky on Linux | Rocky on Ginux |
|---:|---:|---:|
| 1 | 46.721 s | 14.517 s |
| 2 | 34.268 s | 16.032 s |
| 4 | 31.825 s | 15.940 s |

These boot values are repeatable observations, not a kernel-speed comparison.
Linux and Ginux use different firmware, kernel, initramfs, device-discovery,
storage-image, and userspace-startup paths. They answer how long each complete
tested path took under the frozen environment, not why one path was faster.

## Evidence and limits

Machine-readable reduced results are in
[`gate-11-results/`](gate-11-results/). Raw logs remain under
`build/benchmark/rocky10-topology-matrix/` and are not committed because they
include large disposable disk overlays and execution logs.

This matrix characterizes QEMU TCG on one Apple Silicon host. It does not
establish native-hardware performance, broad workload behavior, production
fitness, or general Linux parity.
