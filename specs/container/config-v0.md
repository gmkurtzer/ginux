# Authenticated Container Configuration v0

**Status:** C2 runtime-only profile.

`/etc/ginux-system-profile` is part of the immutable measured root and remains
covered by the signed root-image digest. It describes the available Linux ABI
fixture, but it does not create a container or reserve a container name.

The controller maintains a bounded registry of independently named runtime
containers. Creation applies usable defaults:

```text
container new NAME

init=/sbin/init
cpu=1
memory-mib=128
autostart=false
```

`cpu` accepts any nonzero 16-bit count and `memory-mib` accepts any nonzero
64-bit MiB value. These are per-domain policy values, not admission-time
reservations, and neither has an arbitrary host-wide product ceiling. Runtime
allocation remains constrained by addressability and resources actually
available on the machine.

No image is implied. `container new NAME` therefore creates an incomplete
record until an explicit OCI image reference is configured:

```text
container set alpine image=REGISTRY/REPOSITORY@sha256:DIGEST init=/bin/sh
```

Any number of `FIELD=VALUE` settings that fit on the command line may be
supplied. `image` is required before start and accepts a bounded explicit OCI
reference. The earlier single-setting `container set NAME FIELD VALUE` form
remains accepted. `autostart` accepts `true` or `false`; it is policy rather
than a prerequisite for manual start.

No name is special. The name is chosen by the operator, and lifecycle commands
must use that exact name. The current registry holds up to 32 container records,
each with an independent state, image, init path, and policy. The POC execution
engine runs one container at a time while process/address-space isolation grows
to concurrent execution. Records live only for the current boot; authenticated
persistent configuration is a later contract.

An empty-name query enumerates records. The request `value` is the starting
slot cursor, and the returned `value` is the next occupied slot or
`CONTAINER_CURSOR_END`. A named query continues to perform exact lookup.

Container declarations embedded in the system profile are rejected so a build
cannot silently recreate the former implicit `default` entity.

The runtime record deliberately does not encode networking yet. Per-domain
static and DHCP configuration will extend the versioned record without
granting a container authority over a physical NIC or another domain's
endpoint.
