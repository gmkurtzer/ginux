# Ginux-us

**Ginux Userspace**: a configurable userspace kernel that runs isolated Linux
compatibility domains behind explicit, versioned kernel contracts.

The project is being developed through HDAE gates. See
[`hdae/PROJECT.md`](hdae/PROJECT.md) for intent and scope and
[`hdae/PLAN.md`](hdae/PLAN.md) for the implementation gates.

## Gate 1 development proof

The first gate runs on a macOS arm64 machine and boots a normal Rocky Linux 10
x86-64 host under QEMU TCG. Rocky systemd remains real PID 1. Ginux-us runs as an
ordinary Rocky process, starts a fixed unmodified managed ELF, and uses one
dispatcher to emulate, mediate, or deny its syscalls without letting the
syscall execute directly.

```sh
make doctor
make host-image-fetch  # explicit one-time network operation (~545 MB)
make test              # offline after the Rocky image is cached
make bench-baseline
make demo-gate-1
```

See [`docs/gate-1-foundation.md`](docs/gate-1-foundation.md) for the threat
model, design decision, expected demonstration, benchmark method, and current
limitations.

## Gate 4 domain configuration and lifecycle

Gate 4 adds strict, versioned TOML configuration and the deterministic
single-domain lifecycle. Its demonstration prints the canonical effective
configuration, exercises managed start and cleanup, rejects unsupported or
authority-granting inputs before managed execution, and verifies SIGTERM plus
child reaping inside the Rocky QEMU host.

```sh
make test-config
make test-domain
make demo-gate-4
```

See [`docs/gate-4-domain-lifecycle.md`](docs/gate-4-domain-lifecycle.md) for
the supported schema subset, execution profile, lifecycle, and current
limitations.
