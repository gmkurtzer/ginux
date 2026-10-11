# Network Device Skeleton v0

**Status:** Read-only capability and isolated ring-3 probe verified.

Boot object 4 names the first network-device mediation boundary. The signed
development manifest grants the control plane read authority, and the kernel
may delegate only that authority to an isolated stop-only service. Control ABI
v2 operation 16 returns a fixed descriptor word containing device-contract
version 1, MTU 1500, and one queue. A domain ID or any other non-capability is
rejected before device state is observed.

The QEMU proof creates an independent W^X service domain with only delegated
object-4 read authority. Its ring-3 program first proves denial with an invalid
handle, then probes the device contract, exits with status 42, and has its
capability table and address-space frames retired. A stale handle cannot probe
the device after retirement, and the exact lifecycle generation, service
identity, status, and sequence are consumed before the next workload runs.

This is not yet a network stack or virtual NIC. There is no packet transport,
MAC address, IP configuration, routing, shared ring, interrupt, DMA grant,
driver, or hardware enumeration. Those require separately versioned contracts
and must preserve capability revocation and failure containment.
