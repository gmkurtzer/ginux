# Runtime use cases and acceptance tests

This catalog turns the current architecture into observable behavior. A use
case is considered supported only when the listed automated gate passes against
the real UEFI image; controller-only state simulation is not sufficient.

## UC-1: Boot and administer an empty host

The operator boots Ginux, receives one persistent ring-3 controller, inspects
host processes and network devices, changes link state, and shuts down without
a kernel panic.

- Automated gate: `make test-c1`
- Key observations: controller PID 1 remains stable, DHCP configures `eth0`,
  `lo` is up, virtual links can be created and restarted, and shutdown produces
  `GINUX_KERNEL_OK`.

## UC-2: Start an unmodified Alpine domain

The operator names a Linux-personality domain, assigns an OCI image, and starts
it attached. Ginux resolves the native `linux/amd64` manifest, verifies and
unpacks the image, runs `/bin/sh`, and exposes the image as that domain's root.

- Automated gate: `make test-alpine`
- Key observations: BusyBox and musl execute without image-specific kernel
  toggles; absolute and relative file lookup work; procfs is mounted; detach,
  reattach, clean exit, restart, and failed-init containment all work.

## UC-3: Run independent domains concurrently

The operator runs two long-lived Alpine domains while ingesting and executing
a short Rocky domain. Each running domain retains its own execution, process,
mount, root, and address-space state across controller interactions.

- Automated gate: `make test-multi-domain`
- Key observations: both Alpine PID 1 instances remain independently
  attachable, Rocky exits without disturbing them, one Alpine can stop while
  the other remains running, and QEMU is always reaped by the test harness.

## UC-4: Assign explicit network topology

The operator creates virtual links with physical or virtual uplinks, attaches
one or more links to a domain, and controls each link by the same name from the
controller. A domain without explicit links shares the host network by policy.

- Automated gate today: topology and link lifecycle are covered by
  `make test-c1`; domain attachment rendering is covered there as well.
- Next gate: exchange ICMP and TCP traffic through two simultaneously running
  explicitly attached domains, including independent DHCP leases and DNS.

## UC-5: Contain a domain or service failure

An invalid domain init, denied device operation, or isolated-service fault must
change only the affected object's state. The controller and unrelated domains
remain usable; ordinary userspace failures must not panic the kernel.

- Automated gates: `make test`, `make test-c1`, and `make test-alpine`
- Key observations: failed init is reported as `failed`, service authority is
  revoked before restart, and the controller can continue or shut down cleanly.

## UC-6: Boot a conventional Linux system image

The operator selects a Rocky image with systemd as PID 1 and expects the
domain to boot like a normal machine while remaining inside a native Ginux
domain boundary.

- Diagnostic gate: `make probe-rocky-systemd`
- Current status: not accepted. The Rocky 10 glibc shell and dynamic coreutils
  now pass `make test-rocky-shell`; systemd and the broader general-purpose
  Linux contract remain incomplete.
- Acceptance requires: dynamic mapping metadata, the required Linux syscall
  and pseudo-filesystem surface, working domain networking, interactive access,
  clean stop/restart, and no special Rocky code path in the kernel.

## Compatibility test policy

Linux ABI changes must update the syscall ledger and pass
`make linux-compat-check`. Implemented calls require positive behavior,
boundary/error, and non-mutation-on-failure checks in the measured Linux probe
or system-init probe. OCI/runtime-visible behavior additionally requires an
unmodified image test. Other personalities will receive separate ledgers and
probe suites rather than weakening Linux-specific assertions.

The next high-value end-to-end additions are:

1. Explicit per-domain DHCP, DNS, ICMP, and TCP tests.
2. A controller-driven Linux ABI report sourced from the measured probe.
3. Rocky systemd boot, service startup, attach, stop, and restart.
4. Persistent domain configuration and image reuse across a host reboot.
5. Resource-policy tests for CPU, memory, and I/O guarantees between domains.
