# Protocol ARP Neighbor State v0

**Status:** The measured Rust protocol service learns and refreshes one ARP
neighbor under QEMU/TCG and rejects a conflicting mapping without mutation.

## Parser boundary

`ginux-protocol-state` is a reusable `no_std` crate. Its Ethernet/ARP parser
accepts exactly one 42-byte Ethernet II ARP reply addressed to the configured
local MAC and IPv4 endpoint. It requires Ethernet/IPv4 ARP, reply opcode,
matching Ethernet and ARP sender MACs, a nonzero unicast sender, the configured
peer IPv4, and the configured target MAC and IPv4. Length, EtherType, header,
sender, and target failures are distinguished internally and all fail closed.

## Neighbor rule

State is one entry scoped to one protocol-service generation:

- the first valid reply for the configured peer learns its sender MAC;
- the same IPv4-to-MAC mapping refreshes an overflow-checked observation count;
- a different MAC or peer IPv4 is a conflict and leaves the entry unchanged.

The measured service proves the rule in ring 3. After learning from the first
delivered frame, it constructs a locally conflicting but structurally valid
copy, requires conflict rejection, and verifies that the accepted MAC and count
did not change. The separately delivered second frame must refresh the original
mapping. The service still has packet-read authority only.

## Deliberate limits

This is not a general ARP cache. It has no expiry, replacement, probing,
requests, unsolicited update policy, duplicate-address detection, multiple
interfaces, multiple peers, persistence, or state handoff across service
generations. A conflict is rejected rather than resolved. Administrative stop
discards the generation-local entry with the rest of the service address space.
