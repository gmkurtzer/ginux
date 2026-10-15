# Runtime Domain Configuration v0

**Status:** boot-local controller contract.

The controller maintains independently named Ginux domains. A domain is the
kernel isolation, ownership, accounting, scheduling, device, and failure
boundary. OCI does not create that boundary; it is one way to populate a Linux
personality domain's root filesystem.

Creation applies resource and init defaults but deliberately does not infer a
personality or system image:

```text
domain new NAME

init=/sbin/init
cpu=1
memory-mib=128
autostart=false
```

Before start, the operator must explicitly configure `personality=linux` and
an image. Linux is the only implemented personality in v0; the versioned field
is designed for future BSD and native personalities.

```text
domain set alpine personality=linux image=alpine:latest init=/bin/sh
```

`cpu` accepts any nonzero 16-bit count and `memory-mib` accepts any nonzero
64-bit MiB value. These are domain policy values, not admission-time
reservations. Actual use remains constrained by addressability and resources
available on the machine.

Any number of `FIELD=VALUE` arguments that fit on the command line are
processed. `autostart` is optional and defaults to false. No name is special.
Records live only for the current boot; authenticated persistent configuration
is a later contract.

## Network attachments

The physical `eth0` device is discovered by the kernel. Virtual devices are
created explicitly and may use a physical or virtual uplink:

```text
network new gnet0 type=virtual
network new gnet1 type=virtual uplink=gnet0
```

A domain may attach any number of existing network devices by repeating the
`network` field. An omitted network field means the domain shares the control
domain's host network.

```text
domain set alpine network=gnet0 network=gnet1
```

Attachments are independent kernel objects rather than a singleton domain
attribute. Their implementation is resource-bounded by available kernel
memory; the interface does not impose a one-device product limit. Device names
remain identical in the control domain and attached personality domain.

The v0 control path inventories physical and virtual devices and records
multi-device attachment policy. Per-domain packet queues, addresses, routes,
DHCP, DNS, and Linux interface enumeration must use these attachment objects as
their source of truth as the isolated dataplane is completed.
