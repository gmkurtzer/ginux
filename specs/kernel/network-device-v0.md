# Network Device Skeleton v0

**Status:** Capability-gated link state and isolated ring-3 probe verified.

Boot object 4 names the first network-device mediation boundary. The signed
development manifest grants the control plane read/write authority, and the
kernel delegates that bounded authority to an isolated stop-only service.
Control ABI v2 operation 16 uses `RSI` as a command and `RDX` as its argument.
Read-authorized command 0 requires a zero reserved argument and returns a fixed
descriptor word containing device-contract version 2, MTU 1500, and one queue.
Read-authorized command 1 returns zero for link down or one for link up.
Write-authorized command 2 accepts only zero or one and changes that link state.
Unknown commands, nonzero
reserved arguments, a domain ID, or any other non-capability are rejected
before device state is observed.

The QEMU proof creates an independent W^X service domain with only delegated
object-4 read/write authority. Its ring-3 program proves denial with an invalid
handle, unknown command, and nonzero reserved argument, then probes the device
contract, observes link down, brings it up, observes the change, exits with
status 42, and has its
capability table and address-space frames retired. A stale handle cannot probe
the device after retirement, and the exact lifecycle generation, service
identity, status, and sequence are consumed before the next workload runs.

This is not yet a network stack or virtual NIC. Link state is one kernel-owned
boolean and is not backed by hardware. There is no packet transport, MAC
address, IP configuration, routing, shared ring, interrupt, DMA grant, driver,
or hardware enumeration. Those require separately versioned contracts and must
preserve capability revocation and failure containment.
