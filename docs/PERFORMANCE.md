# Performance Baseline and Gates

Performance claims are tied to an explicit workload, accelerator, and marker.
The Rocky gate measures from entry into `/sbin/init` until the verified console
shell executes. Run it with:

```sh
make benchmark-rocky-systemd
```

The target builds first, uses the persistent verified OCI cache, and selects
QEMU KVM automatically on an x86-64 Linux host with `/dev/kvm`. KVM runs enforce
a 3,000 ms Linux-init-to-console budget. Cross-architecture TCG runs report the
measurement without applying the native budget.

## October 19, 2026 checkpoint

Host: Apple Silicon macOS, QEMU `x86_64` TCG. Image:
`docker.io/rockylinux/rockylinux:10-ubi-init`. Cache: warm and digest verified.

| Measurement | Result |
|---|---:|
| Initial Linux init to verified console | 15,344 ms |
| Optimized repeated runs | 9,495 / 9,810 / 9,953 ms |
| Optimized median | 9,810 ms |
| Median reduction | 36.1% |
| Warm compressed-layer restore | about 2,700 ms |
| Layer unpack and rootfs publication | about 3,500 ms |

The material init delay was a nested child polling a Unix socket owned by its
suspended parent. The former path busy-spun through five- and one-second
timeouts, preventing the parent from making the descriptor ready. The runtime
now yields only when the waited Unix peer is owned by that parent. This removed
about six seconds without weakening general poll timeout behavior.

## Findings closed by measurement

- Rootfs lookup already uses a pathname index. Measured `openat` work was about
  0.3 seconds over the complete boot, so a filesystem rewrite was rejected.
- Saving and restoring the current process runtime snapshot consumed about 65
  ms total. Replacing it solely for this benchmark would add lifecycle risk for
  negligible gain; the general dispatcher remains the architectural route to
  process-owned state.
- Broadly weakening sequentially-consistent atomics and forcing the entire
  kernel to a more aggressive optimization level both caused verification or
  embedded-service regressions. Those speculative changes were reverted. The
  retained timed-wait fast path avoids inactive scans without changing memory
  ordering contracts.
- A cached 299,590,656-byte published Rocky rootfs restored directly in 8,363
  ms. Restoring its 109,755,585 compressed bytes and unpacking them took about
  6,200 ms. The v2 cache can publish a digest-verified rootfs directly, but
  selects that representation only when its size is at most twice the cached
  compressed layers. Rocky therefore keeps the faster layer path. A
  demand-paged disk-backed rootfs is required to remove both full restore and
  full unpack without increasing startup time.

## Platform interpretation

The TCG result measures translated x86 execution on an ARM host; it is not a
native-kernel performance result. The repository configures KVM automatically
and enforces the native three-second gate, but KVM cannot be executed on this
host. That gate must be run on the planned x86-64 Linux system before claiming
the couple-second target is met.
