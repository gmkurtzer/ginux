# Detached-domain scheduler acceptance

The fixtures are built entirely from this checkout. No registry pull, distribution
image, private cache, `volcraft`, or access to the contributor's test host is
required. Use Linux x86-64 with the normal Rust/Clang/QEMU build prerequisites,
GCC with static glibc development files, Python 3, and OVMF. Ubuntu 24.04 provides
these with `gcc clang qemu-system-x86 ovmf`; Rust is pinned by rust-toolchain.toml.

## One-command acceptance

On a Linux host with accessible `/dev/kvm`:

```sh
make test-scheduler SCHEDULER_TEST_ARGS='--accel kvm --output build/scheduler-kvm'
```

Without KVM, use the same TCG command as the public `scheduler` workflow:

```sh
make test-scheduler SCHEDULER_TEST_ARGS='--accel tcg --iterations 5000000 --quiet-seconds 180 --response-timeout 20 --output build/scheduler-tcg'
```

Choose a new output directory for every run. The command builds the normal EFI,
compiles the static PIE payloads, creates a deterministic USTAR rootfs and a new
64 MiB cache through the repository's `seed-benchmark-cache`, then runs all cases.
The private fixture reference `docker.io/library/scheduler-test:local` is resolved
from that cache; it is not a published container image. Network access is needed
only to install build dependencies, not to obtain the test filesystem.

`--cpus 1`, `--cpus 2`, or `--cpus 4` selects a CI matrix cell. `--accel auto`
uses the normal QEMU launcher selection. `QEMU_SYSTEM_X86_64`, `OVMF_CODE`, and
`OVMF_VARS` override platform paths. `GINUX_SCHEDULER_CC` selects an x86-64 Linux
compiler when necessary. Each VM has its own volume, firmware variables and ESP;
the harness terminates only its own QEMU process group and forwards no host ports.

## Required behavior

- A cooperative `sched_yield()` workload responds on every selected topology.
- A no-syscall spinner responds to controller queries and survives three
  stop/start cycles on 1/2/4 CPUs; one CPU also receives 30 consecutive queries.
- Two detached domains preserve GPRs, RCX/R11, CF/DF, stack/red zone, FS/TLS,
  x87 value/control, MXCSR and YMM0/7/15, with distinct seeds per domain/worker.
  Hardware CPUID masks must be 1/3/15. Both payloads must report success and
  exit cleanly after console resume.
- Four sibling threads in each of two domains compete on one CPU.
- SIGUSR1 returns from the same assembly check; both domains complete exactly
  one handler and five successful timed sleeps, then exit cleanly.
- A continuous four-worker workload is stopped/restarted three times on four
  CPUs. It does not finish before the stop operation.

Startup waits for each domain to enter userspace before starting the next; this
avoids overlapping the kernel's asynchronous cache/install request slot. After
both starts, the context harness sends **no serial input during the quiet
interval**. In addition to the sentinel results it requires at least 500 ms of
overlap between the two reported computation intervals. Sequential completion
cannot satisfy that assertion. Context checks must individually last at least
500 ms; the signal report must also include `signals=1 waits=5`.

KVM uses 100 million assembly iterations by default. TCG uses 5 million to keep
CI bounded, while retaining identical sentinel checks and the duration/overlap
requirements. These are correctness tests, not performance comparisons. A fast
emulator that makes a case too short must use more iterations, not remove the
minimum-duration assertion.

## Negative control

The public workflow also builds pre-fix revision `7fdc880e` (after #84, before
the controller-desktop merge and S-1) for the one-CPU cell.
The **same** cache and current harness must observe a responsive cooperative
workload and no response from its no-syscall spinner. Supply any separately
built unpatched checkout with:

```sh
make test-scheduler SCHEDULER_TEST_ARGS='--accel tcg --iterations 5000000 --quiet-seconds 180 --response-timeout 20 --output build/scheduler-control --negative-source /absolute/path/to/unpatched-checkout'
```

This control never changes or disables the acceptance checks in the candidate
kernel. A failed cooperative control invalidates the reproduction and fails the
suite. A timeout during firmware boot, domain creation or configuration is an
error rather than evidence of the scheduling defect.

## Evidence and individual cases

`summary.json` records every command and exit status plus the EFI hash and source
identity. `fixtures/fixtures.json` records compiler identity, loop count, rootfs
and payload hashes. Each case retains a serial log and timestamped JSON events.
Public CI uploads JSON and logs even on failure; generated volumes and private
host files are not uploaded. Any failed or missing required case fails the job.

For a focused rerun, `prepare.py --output NEW_DIRECTORY` creates the cache, then
`probe_responsiveness.py` and `probe_context.py` accept `--source`, `--volume`,
`--output`, `--accel`, and `--cpus`. See their `--help` for topology, signal,
lifecycle and timing arguments. Historical KVM evidence is documented in
[`docs/scheduler/REVIEW.md`](../../docs/scheduler/REVIEW.md).

## Host checks

```sh
make check-scheduler
cargo test --package ginux-kernel-policy
```

The host harness includes the real native scheduler with platform/domain stubs.
It exercises both overflow boundaries and 1024 controller lifecycle handoffs.
