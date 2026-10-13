# Native Protocol Service Bootstrap ABI v2

**Status:** Shared Rust and C layouts are exercised by a measured supervised
protocol service under QEMU/TCG.

Version 2 retains the 56-byte v1 layout. It requires the exact packet-consumer,
Ethernet/ARP, wait-loop, and supervised-loop feature set. `packet_count` is zero
to select an indefinite exact-sequence loop; all other fields retain their v1
offsets and meaning.

The service waits for, consumes, and validates consecutive packet sequences
until its domain is stopped or a fatal validation error occurs. Sequence
exhaustion fails closed. Zero is a mode selector, not an unbounded resource
grant: the FIFO, copied frame, call arguments, service mappings, capabilities,
and scheduler records remain independently bounded.

Normal shutdown is external. The service owns no management capability and
cannot authorize its own continued lifetime. A supervisor with the domain's
stop authority cancels the blocked packet wait, revokes packet authority,
releases the address space, scrubs its frames, and publishes a stopped
lifecycle event. v2 does not yet define a graceful drain request, protocol
state export, restart handoff, or network configuration update.
