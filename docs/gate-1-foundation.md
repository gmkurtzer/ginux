# Gate 1 Rocky host and interception proof

Gate 1 is a constructive development proof for the Ginus userspace kernel. It
does not claim to be a production isolation boundary. It proves the host model
and the first mediation invariant: a normal Rocky Linux host boots first,
Ginus runs as an ordinary process, and selected operations from an unmodified
x86-64 ELF reach Ginus before Linux can execute them.

## Reference environment

- Development machine: macOS on arm64.
- Emulated host: Rocky Linux 10.2 x86-64 under QEMU TCG, with host systemd as
  real PID 1.
- Host image: official immutable
  `Rocky-10-GenericCloud-Base-10.2-20260525.0.x86_64.qcow2`, verified against
  Rocky's published SHA-256
  `9fc9e9ff16888bb68ac39b0392e25c9c92684d50c85f1cce6ab549363bbc4b48`.
- Ginus: static Rust x86-64 ELF copied into the running Rocky host and invoked
  manually as the unprivileged `ginus` user.
- Acceleration: QEMU TCG only. KVM is not required or used.

`make host-image-fetch` is the only Gate 1 target that uses the network. QEMU
guest networking is restricted during ordinary verification. Generated
images, keys, overlays, logs, benchmark results, and binaries remain under
`build/` or `target/` and are not committed. `make clean` preserves the
verified source-image cache.

## Host and domain boundary

```text
Rocky host systemd (real PID 1)
└── Ginus (ordinary host process)
    └── managed program
```

Ginus does not replace, reboot, or shut down the Rocky host. The Gate 1 test
waits for host systemd readiness before starting Ginus and checks host systemd
again after Ginus exits. A later gate will run Rocky systemd beneath Ginus as
virtual domain PID 1; that does not change the real host PID 1.

## Threat model for this proof

The managed payload is untrusted. The Rocky host kernel, QEMU, Ginus, approved
configuration, and macOS machine are trusted for this gate. The payload has its
own Linux process address space and is traced from its first instruction after
`execve`.

The tested properties are narrow and falsifiable:

1. Ginus refuses to run as host PID 1.
2. Every syscall issued by the fixed single-threaded payload stops at Ginus.
3. Ginus resolves it through `ginus_core::decide` before any host effect.
4. Ginus either emulates a result, performs checked mediation, or returns a
   denial. There is no direct-continue decision.
5. A managed `write` carries a unique marker. The test fails if that marker is
   observed in host output.
6. After Ginus exits, Rocky systemd and `multi-user.target` remain healthy.

This proof does not yet provide general ELF loading, a Ginus-owned address
space, threads, signals, dynamic linking, domain systemd, virtual filesystems,
or a complete syscall ABI. Linux `ptrace` is a bootstrap mechanism for the
experiment, not the final Ginus execution architecture.

## Execution-platform decision

Gate 1 uses x86 `PTRACE_SYSEMU`. At syscall entry, Linux stops the tracee
without executing the syscall. Ginus reads the register set, calls the central
dispatcher, writes the Linux-style result, and resumes to the next syscall
entry. This works as an ordinary process under QEMU TCG and requires neither
KVM nor a nested guest kernel beneath the domain.

Seccomp user notification was not selected because it is primarily a host
mediation facility and does not by itself establish the application-kernel
task and memory model Ginus needs. Full CPU instruction emulation was not
selected for this first experiment because Gate 1 only needs to establish the
host/process boundary and syscall interception. Gate 3 will use this evidence
when implementing the Ginus execution kernel.

## Automated and human verification

After the one-time image fetch:

```sh
make host-boot-test
make test
make bench-baseline
make demo-gate-1
```

The demo must show Rocky readiness with systemd as PID 1, followed by:

```text
GINUS: host-process pid1=no
GINUS: syscall=getpid decision=emulate result=4242
GINUS: syscall=write decision=mediate host-write=not-executed
GINUS: syscall=uname decision=deny errno=EPERM
GINUS: direct-host-syscall bypass=not-observed
GATE1: PASS
HOST: healthy after Ginus exit
```

The marker `UNMEDIATED_PAYLOAD_WRITE` must not appear. The demonstration exits
nonzero if Rocky or cloud-init does not become ready, host PID 1 is not
systemd, an expected Ginus line is missing, the marker appears, host health is
lost, or VM cleanup fails.

## Baseline benchmark method

`make bench-baseline` runs the same three workloads inside the Rocky VM:

- `cpu`: five million deterministic integer/rotate iterations;
- `syscall`: one hundred thousand `getpid` calls;
- `process`: sixty-four `fork`/`wait4` child startups.

Each workload runs five times directly on the Rocky host kernel and five times
inside a conventional Linux namespace boundary using mount, PID, UTS, and IPC
namespaces. Timing uses the guest monotonic clock and nanoseconds. The target
writes all samples to `build/bench-baseline.csv` and reports count, mean,
population variance, and standard deviation in
`build/bench-baseline-summary.csv`. These controls characterize measurement
noise and namespace cost; they do not yet measure Ginus overhead or set a
performance budget.
