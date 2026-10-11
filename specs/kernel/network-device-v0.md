# Network Device Skeleton v0

**Status:** Capability-gated link state and isolated ring-3 probe verified.

Boot object 4 names the first network-device mediation boundary. The signed
development manifest grants the control plane read/write authority, and the
kernel delegates that bounded authority to an isolated stop-only service.
Control ABI v2 operation 16 uses `RSI` as a command and `RDX` as its argument.
Read-authorized command 0 requires a zero reserved argument and returns a fixed
descriptor word containing device-contract version 8, MTU 1500, and one queue.
Read-authorized command 1 returns zero for link down or one for link up.
Write-authorized command 2 accepts only zero or one and changes that link state.
Write-authorized command 3 stores one packed IPv4 address and prefix: bits 0-31
hold the address, bits 32-39 hold a prefix from 0 through 32, and all higher
bits must be zero. Address zero is accepted only with prefix zero to clear the
configuration. Read-authorized command 4 requires argument zero and returns the
packed value. An effective IPv4 change also clears a gateway that is not usable
inside the new prefix, and the combined update advances the configuration
generation only once. The ring-3 proof rejects prefix 33 before installing the
valid configuration, then moves from `10.0.2.15/24` to `10.0.4.15/24` and
requires the old gateway to be cleared.
Write-authorized command 5 stores a single default IPv4 gateway; zero clears
it, while a nonzero gateway must be a distinct, usable host address within the
configured prefix. Read-authorized command 6 requires argument zero and returns
that value. The proof rejects off-subnet `10.0.3.2`, then installs and reads
back `10.0.2.2`.
Read-authorized command 7 requires argument zero and returns a monotonic,
saturating configuration generation. Effective link, IPv4, and gateway changes
advance it; rejected writes and accepted writes of an already-current value do
not. The ring-3 proof makes three effective changes, repeats the link-up write,
and requires generation three. The current single-CPU call gate serializes
mutations and observations.
Write-authorized command 8 copies a 16-byte static-route record from the caller:
a little-endian destination, one prefix byte, three zero reserved bytes, a
little-endian next-hop, and four more zero reserved bytes. A nonempty record
must have a canonical destination with prefix 1 through 32 and a distinct,
usable on-link next-hop; an all-zero record clears it. Read-authorized command 9
copies the record to caller-owned writable memory and returns 16. The proof
rejects noncanonical `192.168.50.1/24`, installs `192.168.50.0/24` via
`10.0.4.2`, reads it back, and observes exactly one generation advance.
Read-authorized command 10 copies one coherent, version-1 56-byte snapshot to
caller-owned writable memory. It contains seven little-endian 64-bit words:
snapshot version, generation, link, IPv4 configuration, default gateway, packed
route destination/prefix, and route next-hop. The ring-3 proof validates every
word. Coherence currently relies on the documented single-CPU, non-nested call
gate and must be revisited before concurrent device calls are introduced.
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

Before exit the service also installs and reads back `10.0.2.15/24`. This is not
yet a network stack or virtual NIC. Link and IPv4 state are kernel-owned scalar
values and are not backed by hardware. There is no packet transport, MAC
address, general route table, shared ring, interrupt, DMA grant, driver, or hardware
enumeration. Those require separately versioned contracts and must preserve
capability revocation and failure containment.
