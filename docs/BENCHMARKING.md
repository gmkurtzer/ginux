# Linux and Ginux Comparative Benchmark

This harness tests two deliberately narrow questions: how long do Linux and
Ginux take to reach the same systemd readiness marker from QEMU process launch,
and how do they execute the same x86-64 userspace workload?

It does not establish general operating-system performance. Ginux is currently
single-vCPU, the macOS development host can only use QEMU TCG for x86-64, and
the two systems do not yet expose equivalent storage stacks.

## Fixed inputs

- QEMU machine: `q35`
- vCPUs: 1
- memory: 2 GiB
- CPU: the same QEMU CPU string for both guests
- accelerator: KVM on x86-64 Linux when available, otherwise TCG
- workload platform: `linux/amd64`
- workloads: `sysbench` CPU, memory, threads, and mutex tests
- repetitions: three boots and three workload runs by default

The Linux baseline is the official Rocky Linux 10.2 GenericCloud image
`Rocky-10-GenericCloud-Base-10.2-20260525.0.x86_64.qcow2`, pinned to SHA-256
`9fc9e9ff16888bb68ac39b0392e25c9c92684d50c85f1cce6ab549363bbc4b48`.
The source URL and all other fixed settings are in
`profiles/benchmarks/rocky10-single-vcpu.env`.

The Ginux guest uses `rockylinux/rockylinux:10-ubi-init`. GenericCloud and UBI
are different Rocky image profiles, so a systemd boot result is not a
byte-for-byte userspace comparison. Workload results are accepted only when
both guests report the same SHA-256 digest for `/usr/bin/sysbench`.

Image download, OCI transfer, package installation, and one-time cloud-image
preparation are excluded from workload timing. Linux measurement runs use
disposable qcow2 overlays. Benchmark data uses a dedicated virtio serial port,
separate from the login and boot console.

## Commands

```sh
make benchmark-prepare-linux
make benchmark-prepare-ginux
make benchmark-boot-comparison
make benchmark-ginux-sysbench
make benchmark-compare
```

The first command downloads and verifies the pinned image, then installs EPEL
and `sysbench` once. Generated images and raw logs live under
`build/benchmark/rocky10-single-vcpu/` and are not source artifacts.

Environment variables can shorten exploratory runs without changing the
profile defaults:

```sh
BENCHMARK_BOOT_RUNS=1 BENCHMARK_WORKLOAD_RUNS=1 \
  BENCHMARK_WORKLOAD_SECONDS=1 make benchmark-linux-comparison
```

## October 23, 2026 matched boot checkpoint

Host: Apple Silicon macOS. Guest ISA: x86-64. Accelerator: QEMU TCG.

Every value below starts immediately before QEMU process launch and ends at the
first systemd `multi-user.target` marker. Both use `q35`, one vCPU, 2 GiB RAM,
the `max,+smep,+smap,+umip` CPU model, TCG, and warm local image data.

| System | Run 1 | Run 2 | Run 3 | Median | Mean | Range |
|---|---:|---:|---:|---:|---:|---:|
| Rocky on Linux | 41.596 s | 46.472 s | 42.579 s | 42.579 s | 43.549 s | 4.876 s |
| Rocky on Ginux | 12.910 s | 12.820 s | 12.860 s | 12.860 s | 12.863 s | 0.090 s |

Ginux reached the matched marker 3.31 times sooner, corresponding to 69.8%
less median elapsed time in this configuration. Linux varied materially across
the three TCG runs (an 11.5% range relative to its median), while Ginux's range
was 0.7%. This is a preliminary operational full-guest startup comparison, not
a general kernel-performance result.

The likely explanation is structural. Ginux boots a compact EFI kernel and
controller, restores a cached OCI filesystem, and launches the Rocky domain
without booting a second kernel or initramfs. Rocky Linux follows its
general-purpose UEFI, kernel, initramfs, device discovery, and GenericCloud
path. Ginux also implements less hardware and service machinery, so omitted
work may appear as lower latency. The Linux GenericCloud and Ginux UBI profiles
do not enable identical services; their qcow2 and OCI/raw storage paths also
differ. TCG can affect their instruction paths differently. A stronger claim
requires an identical userspace profile, more runs, equivalent storage paths,
and execution on the same x86-64 KVM host.

Raw boot results are written to `rocky-linux-summary.tsv`,
`ginux-boot-summary.tsv`, and `boot-comparison.tsv` under the benchmark build
directory.

## October 23, 2026 matched sysbench checkpoint

Rocky on Linux used the official Rocky Linux 10.2 GenericCloud image and its
stock `6.12.0-211.16.1.el10_2.0.1.x86_64` kernel. Rocky on Ginux used the Rocky
Linux 10 UBI init image as a domain. Both guests ran the same Rocky Linux 10
x86-64 sysbench executable, verified
by SHA-256
`7cf6b71521436bd32a8ab197851035ff1632bee383fec5a35468d61969e7c18d`.
Commands, QEMU machine, CPU count, memory size, CPU model, accelerator, and
three-second workload duration matched. The table reports nine observations
per workload and system.

| Workload | Metric | Rocky on Linux | Rocky on Ginux | Relative result |
|---|---|---:|---:|---:|
| CPU | events/s | 232.360 | 267.650 | Ginux 1.152x faster |
| Memory | MiB/s | 679.510 | 620.580 | Ginux 1.095x slower |
| Threads | events/s | 306.129 | 309.732 | Ginux 1.012x faster |
| Mutex | seconds/event | 1.550 | 1.423 | Ginux 1.089x faster |

The CPU workload is almost entirely userspace computation; its 12.8% Ginux
lead is preliminary and may include TCG variation or different background
work. The memory workload uses one-kibibyte events and times each event. A
block-size sweep reached 2.76 GiB/s with one-mebibyte events, showing that the
original 48.390 MiB/s result measured repeated `clock_gettime` overhead rather
than raw memory bandwidth. A trusted-copy path, removal of clock calls from
the diagnostic syscall ring, and a guarded clock fast path raised the matched
median to 233.210 MiB/s. A Linux-compatible vDSO and adjacent read-only vvar
page then removed the remaining privilege crossings and raised the median to
584.540 MiB/s. A second vDSO pass replaced per-read division and synchronized
live snapshots with an immutable per-exec time base, fixed-point TSC
conversion, and constant-time reciprocal normalization. The final median is
620.580 MiB/s. Rocky on Ginux is 1.095 times slower in this one-kibibyte test;
the remaining TCG gap is too small to assign to one kernel mechanism without
native KVM profiling.

The original threads checkpoint exposed a scheduler defect: every
worker `sched_yield()` selected a just-preempted busy supervisory thread, so
the worker waited for another timer interrupt on each of the workload's 1,000
yields per event. Tracking the last preempted thread and excluding it from an
immediate voluntary-yield selection raised the Ginux median from 0.078 to
181.915 events/s. A second pass found that a yield with no other runnable
thread still serialized and restored the unchanged user context. Treating
that Linux scheduling hint as a no-op raised the median to 309.732 events/s,
1.012 times the Rocky result. This closes the measured single-thread yield
gap; it does not measure parallel execution or SMP scheduling.
The single-thread mutex workload completed one event on both kernels and
carries little weight.

Ginux samples use a fresh QEMU boot for isolation because the current shell
child-lifecycle path does not reliably support several sequential sysbench
children. Workload timing excludes those boots. Complete output is preserved
in `rocky-linux-run-*-results.log`, `ginux-sysbench-*.log`, and
`sysbench-comparison.tsv`.

## Publication requirements

Before treating these checkpoint results as general performance evidence:

1. Profile the remaining 1.095x memory-test gap under native x86-64 KVM.
2. Make `domain exec --interactive` retain the running systemd domain and
   deliver its PTY and output.
3. Repeat both sides on the same x86-64 Linux host with KVM.
4. Add dispersion and confidence intervals; do not mix TCG and KVM results.
5. Add equivalent IPC, network, and storage tests.

File-I/O tests are intentionally deferred until equivalent storage paths can be
defined. Multicore tests are deferred until Ginux has SMP support.
