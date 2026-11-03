# Gate 1 foundation and interception proof

Gate 1 is a constructive development proof for the Ginus userspace kernel. It
does not claim to be a production isolation boundary. Its purpose is to prove
the first architectural invariant in the required development environment:
selected operations from an unmodified x86-64 ELF reach Ginus before Linux can
execute them.

## Reference environment

- Host: macOS on arm64.
- Machine: QEMU `pc` with TCG emulation and an x86-64 CPU.
- Test kernel: Alpine Linux 3.24.3 `virt`, version 6.18.58, fetched from an
  immutable release URL and verified as SHA-256
  `17fbd3efd16f2e7cb1b945f06423b37399ba7d5ac8cdd3665e7f13f363500e85`.
- Ginus: static Rust x86-64 ELF installed as `/sbin/ginus` in a generated
  initramfs and selected with `rdinit`, making it Linux PID 1.
- Acceleration: QEMU TCG only. KVM is not required or used.

`make fetch-test-assets` is the only Gate 1 target that uses the network.
Generated images, logs, benchmark results, and binaries remain under `build/`
or `target/` and are not committed.

## Threat model for this proof

The managed payload is untrusted. The emulated Linux kernel, QEMU, Ginus, and
the macOS host are trusted for this gate. The payload has its own Linux process
address space and is traced from its first instruction after `execve`.

The properties under test are narrow and falsifiable:

1. Every syscall issued by the single-threaded payload stops at Ginus.
2. Ginus resolves it through `ginus_core::decide` before any host effect.
3. Ginus either emulates a result, performs a checked mediation, or returns a
   denial. There is no direct-continue decision.
4. A managed `write` carries a unique marker. The test fails if that marker is
   observed on the host console, proving the attempted direct effect did not
   occur in the tested path.
5. Unknown operations and process/thread creation are denied by the central
   dispatcher, so this fixed payload cannot create an untraced execution path.

This proof does not yet provide general ELF loading, a Ginus-owned address
space, threads, signals, dynamic linking, systemd, virtual filesystems, or a
complete syscall ABI. Those belong to later gates. Linux `ptrace` is a bootstrap
mechanism for the experiment, not a claim that Linux tracing alone is the final
Ginus architecture.

## Execution-platform decision

Gate 1 uses x86 `PTRACE_SYSEMU`. At syscall entry, Linux stops the tracee
without executing the syscall. Ginus reads the register set, calls the central
dispatcher, writes the Linux-style result, and resumes to the next syscall
entry. This mechanism works under QEMU TCG and requires neither KVM nor a nested
guest kernel.

Seccomp user notification was not selected because it is primarily a host
mediation facility and does not by itself establish the application-kernel
task and memory model Ginus needs. Full CPU instruction emulation was also not
selected for this first experiment because it would obscure the narrower
syscall-boundary question with a much larger implementation. Gate 3 will use
this evidence when implementing the actual Ginus execution kernel.

## Automated and human verification

After the one-time asset fetch:

```sh
make test
make demo-gate-1
```

The demo must show:

```text
GINUS: syscall=getpid decision=emulate result=4242
GINUS: syscall=write decision=mediate host-write=not-executed
GINUS: syscall=uname decision=deny errno=EPERM
GINUS: direct-host-syscall bypass=not-observed
GATE1: PASS
cleanup: QEMU exited and no background process remains
```

The marker `UNMEDIATED_PAYLOAD_WRITE` must not appear. The demonstration exits
nonzero if an expected line is missing, the marker appears, QEMU times out, or
the guest reports failure.

## Baseline benchmark method

`make bench-baseline` runs the same three workloads in the same QEMU boot:

- `cpu`: five million deterministic integer/rotate iterations;
- `syscall`: one hundred thousand `getpid` calls;
- `process`: sixty-four `fork`/`wait4` child startups.

Each workload runs five times directly on the emulated Linux kernel and five
times inside a conventional Linux namespace boundary using mount, PID, UTS,
and IPC namespaces. Timing uses the guest monotonic clock and nanoseconds. The
target writes all samples to `build/bench-baseline.csv` and reports count,
mean, population variance, and standard deviation in
`build/bench-baseline-summary.csv`. These controls characterize measurement
noise and namespace cost; they do not yet measure Ginus overhead or set a
performance budget.
