# Linux and Ginux Comparative Benchmark

This harness tests a deliberately narrow question: how does the current Ginux
Linux personality compare with Linux when both execute the same x86-64
userspace workload in closely matched QEMU virtual machines?

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
make benchmark-linux-comparison
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

## October 23, 2026 TCG checkpoint

Host: Apple Silicon macOS. Guest ISA: x86-64. Accelerator: QEMU TCG.

| Measurement boundary | Linux time | Ginux time |
|---|---:|---:|
| Full guest boot: QEMU process launch to multi-user/login readiness | 42.666 s median (42.807 / 41.901 / 42.666) | Not measured |
| Userspace init: PID 1 entry to verified console/login | Not measured | 4.452 s (one run) |
| Warm workload launch: domain start request to login | Not applicable | About 10.957 s, derived (one run) |
| Matched sysbench: same binary; CPU, memory, threads, and mutex | 36/36 runs completed | No accepted runs |

The separately observed 34.64-second Linux uptime at readiness includes both
kernel and userspace execution, so it is not presented as a userspace-init
comparison. The warm Ginux domain figure adds the measured
6.505 seconds for cached OCI restoration, unpacking, and publication to the
4.452-second init-to-console segment. It excludes Ginux firmware, kernel, and
controller startup because those components were already running when the
domain start command was issued.

All 36 Linux workload executions completed successfully. Their sysbench binary
digest was
`7cf6b71521436bd32a8ab197851035ff1632bee383fec5a35468d61969e7c18d`.
The raw logs preserve the complete sysbench output.

The Ginux Rocky domain continued to reach its verified console in 4.45 seconds
from Linux-init entry. This does **not** mean that Ginux booted approximately
ten times faster than Linux: 42.666 seconds covers a complete Linux VM launch,
while 4.45 seconds covers only the final systemd segment inside Ginux. A
comparative sysbench result is **not yet available**:
the current domain-exec path does not deliver an interactive PTY after systemd
has reached multi-user, and repeated non-interactive exec caused the running
domain to become unavailable. The comparison gate therefore fails rather than
publishing unmatched or fabricated numbers. This is now a concrete Linux ABI
and domain-lifecycle test requirement.

The apparent timing difference is most plausibly explained by the mismatched
boundaries. Ginux reuses an already-running kernel and controller and starts a
cached, narrower UBI userspace. The Linux path includes UEFI, kernel and device
initialization, and a GenericCloud userspace. Ginux also implements less
hardware and service machinery, so some omitted work appears as lower latency.
TCG translation and the different Rocky image profiles add further confounders.
No relative boot-performance claim is supported until both measurements start
at QEMU launch and end at the same readiness marker under the same cache and
userspace policy.

## Publication requirements

Before citing comparative workload results:

1. Make `domain exec --interactive` retain the running systemd domain and
   deliver its PTY and output.
2. Complete every Ginux workload repetition without a domain failure.
3. Pass `make benchmark-compare`, including exact sysbench binary identity.
4. Repeat both sides on the same x86-64 Linux host with KVM.
5. Report medians and dispersion; do not mix TCG and KVM results.

File-I/O tests are intentionally deferred until equivalent storage paths can be
defined. Multicore tests are deferred until Ginux has SMP support.
