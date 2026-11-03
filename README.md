# Ginus

**Ginux Userspace**: a configurable userspace kernel that runs isolated Linux
compatibility domains behind explicit, versioned kernel contracts.

The project is being developed through HDAE gates. See
[`hdae/PROJECT.md`](hdae/PROJECT.md) for intent and scope and
[`hdae/PLAN.md`](hdae/PLAN.md) for the implementation gates.

## Gate 1 development proof

The first gate runs on a macOS arm64 host and boots an x86-64 Linux test kernel
under QEMU TCG. Ginus becomes PID 1, starts a fixed unmodified test ELF, and
uses one dispatcher to emulate, mediate, or deny its syscalls without letting
the syscall execute directly.

```sh
make doctor
make fetch-test-assets  # explicit one-time network operation
make test               # offline after the test kernel is cached
make bench-baseline
make demo-gate-1
```

See [`docs/gate-1-foundation.md`](docs/gate-1-foundation.md) for the threat
model, design decision, expected demonstration, benchmark method, and current
limitations.
