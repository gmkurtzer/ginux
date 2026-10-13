# Authenticated Container Configuration v0

**Status:** C2 runtime-only profile.

`/etc/ginux-system-profile` is part of the immutable measured root and remains
covered by the signed root-image digest. It describes the available Linux ABI
fixture, but it does not create a container or reserve a container name.

The C2 controller creates one bounded runtime container explicitly:

```text
container new NAME
container set NAME init /sbin/init
container set NAME cpu 1
container set NAME memory-mib 128
container set NAME autostart true
```

No name is special. The name is chosen by the operator, and lifecycle commands
must use that exact name. The current bounded implementation accepts one
container, the measured `/sbin/init`, one virtual CPU, 128 MiB, and an
`autostart` value of `true`. All four settings are required before start. The record lives only for
the current boot; authenticated persistent configuration is a later contract.

Container declarations embedded in the system profile are rejected so a build
cannot silently recreate the former implicit `default` entity.

The runtime record deliberately does not encode networking yet. Per-domain
static and DHCP configuration will extend the versioned record without
granting a container authority over a physical NIC or another domain's
endpoint.
