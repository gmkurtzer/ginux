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
| Final repeated runs | 9,935 / 10,226 / 10,226 ms |
| Final median | 10,226 ms |
| Current repeated runs | 4,081 / 4,083 ms |
| Current median | 4,082 ms |
| Reduction from initial baseline | 73.4% |
| Warm compressed-layer restore | about 2,700 ms |
| Layer unpack and rootfs publication | about 3,500 ms |

The material init delay was a nested child polling a Unix socket owned by its
suspended parent. The former path busy-spun through five- and one-second
timeouts, preventing the parent from making the descriptor ready. The runtime
now yields only when the waited Unix peer is owned by that parent. This removed
about six seconds without weakening general poll timeout behavior.

The current pass added an opt-in kernel profiler (`make
profile-rocky-systemd`) and measured syscall, fork/exec, scheduler, page-table,
and frame-lifecycle phases. It found quadratic cross-process frame-reference
scans and eager duplication of immutable rootfs pages. Exact transient frame
indexes removed the quadratic scans. A per-domain, dynamically resized
immutable file-page cache now shares read-only and executable rootfs pages;
adding write permission first creates a private copy. The measured short boot
now allocates about 10,400 file frames instead of about 172,000 while retaining
zero-before-reuse for private frames.

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
  ordering contracts. Bank selection now uses an acquire-load fast path after
  one-time initialization, avoiding a redundant locked compare-and-swap on
  every per-domain atomic access. Its 9,935 / 10,226 / 10,226 ms TCG result is
  within run-to-run variance, so no speedup is claimed.
- A cached 299,590,656-byte published Rocky rootfs restored directly in 8,363
  ms. Restoring its 109,755,585 compressed bytes and unpacking them took about
  6,200 ms. The v2 cache can publish a digest-verified rootfs directly, but
  selects that representation only when its size is at most twice the cached
  compressed layers. Rocky therefore keeps the faster layer path. A
  demand-paged disk-backed rootfs is required to remove both full restore and
  full unpack without increasing startup time.

## Optimization-plan disposition

| Item | Disposition |
|---|---|
| Filesystem/descriptor syscall paths | Indexed lookup retained; measured total `openat` cost is about 0.3 seconds. |
| Excessive global synchronization | Redundant bank-initialization CAS removed while preserving acquire/release publication. |
| Full process-switch snapshots | Replacement rejected at this checkpoint: measured total cost is about 65 ms and the current ownership rewrite has materially higher lifecycle risk. |
| Native x86-64 KVM | Gate implemented; execution remains pending on a KVM-capable host. |
| Direct cached OCI reuse | Implemented with digest verification and adaptive selection; Rocky layers remain faster than its 299 MB direct image. |
| Immutable rootfs file pages | Dynamically cached per domain with exact file/offset identity and copy-on-write for writable protection changes. |
| Selective hot compilation | Kernel release package uses `opt-level = 3`; size-sensitive embedded controller remains `opt-level = "z"`. |

## Platform interpretation

The TCG result measures translated x86 execution on an ARM host; it is not a
native-kernel performance result. The repository configures KVM automatically
and enforces the native three-second gate, but KVM cannot be executed on this
host. The installed QEMU binary reports only the `tcg` accelerator. The current
four-second TCG result meets this checkpoint's few-second target; the native
three-second gate still must run on the planned x86-64 Linux system.

## Verification status

`make check` passes, including the architecture check, the Linux compatibility
matrix, the native service ABI check, and all host-side unit tests. The Rocky
systemd benchmark reaches its verified console login on every measured run.

The verification boot now advances past its formerly stale epoll and `statx`
assertions, but still fails in the post-exec system-supervisor lifecycle probe.
That is a separate correctness item; it is not counted as a successful full
`make test` result in this checkpoint.
