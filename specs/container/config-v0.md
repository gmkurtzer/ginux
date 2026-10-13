# Authenticated Container Configuration v0

**Status:** C2 minimal profile.

`/etc/ginux-containers` is part of the immutable measured root and therefore
covered by the signed root-image digest. The v0 profile accepts exactly one
newline-terminated record:

```text
GINUX_CONTAINERS_V0
container	default	autostart	/sbin/init	cpu=1	memory_mib=128
```

The record names the stable container `default`, requires autostart, selects
the measured `/sbin/init`, and grants one virtual CPU and 128 MiB as bounded
initial policy. Unknown versions, fields, records, paths, or values fail the
build and fail closed again when the kernel validates the published root.

This format deliberately does not encode networking yet. Per-domain static
and DHCP configuration will extend the versioned record without granting a
container authority over a physical NIC or another domain's endpoint.
