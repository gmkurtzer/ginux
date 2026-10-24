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
| Rocky Linux | 41.596 s | 46.472 s | 42.579 s | 42.579 s | 43.549 s | 4.876 s |
| Ginux with Rocky domain | 12.910 s | 12.820 s | 12.860 s | 12.860 s | 12.863 s | 0.090 s |

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

All 36 Linux workload executions completed successfully. Their sysbench binary
digest was
`7cf6b71521436bd32a8ab197851035ff1632bee383fec5a35468d61969e7c18d`.
The raw logs preserve the complete sysbench output.

A comparative sysbench result is **not yet available**:
the current domain-exec path does not deliver an interactive PTY after systemd
has reached multi-user, and repeated non-interactive exec caused the running
domain to become unavailable. The comparison gate therefore fails rather than
publishing unmatched or fabricated numbers. This is now a concrete Linux ABI
and domain-lifecycle test requirement.

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
