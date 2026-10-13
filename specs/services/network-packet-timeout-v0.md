# Native Network Packet Wait Timeout v0

**Status:** A ring-3 packet consumer expires through a calibrated local-APIC
deadline and a replacement consumes the unchanged sequence under QEMU/TCG.

Operation 24 is the bounded form of packet wait operation 23. It requires the
same packet-object `READ` authority and 32-byte packet request v0 in `RSI`.
`RDX` supplies a relative timeout in whole milliseconds from zero through
1,000. The kernel checks packet state before the timeout: an available exact
sequence returns immediately, and a stale or future sequence returns `STALE`.
For an empty exact-next sequence, zero milliseconds returns `TIMED_OUT`
immediately without registering a wait. A positive value registers the exact
wait and arms a checked local-APIC one-shot. Larger values are rejected.

Publication and expiry race through the same typed scheduler wait. Publication
first completes the bounded packet copy and sequence allocation, makes the
consumer ready with the exact sequence, and cancels the matching timer. If the
deadline wins, the interrupt makes the consumer ready with `TIMED_OUT`, removes
the exact-sequence claim, and may preempt a non-cooperative ring-3 peer. Expiry
does not allocate, consume, or advance a packet sequence. External control-
plane STOP also disarms a matching packet timer while retiring the domain.

The executable proof rejects a 1,001 ms request, performs a zero-timeout query,
then blocks for 10 ms while another domain spins without yielding. The APIC
interrupt preempts that peer, the consumer observes `TIMED_OUT`, both domains
retire, and the wait key becomes available with the sequence unchanged. A
fresh consumer claims that sequence. Its separate 1,000 ms timed wait is
satisfied by publication, and the proof requires the pending timer to be
cancelled without recording another timeout.

## Limits

Timeouts are relative, millisecond-granularity, and limited to one second.
The bootstrap processor currently has one local-APIC timer owner, so only one
timed native packet wait can be armed at once. There is no absolute deadline,
self-cancellation call, readiness set, multi-object wait, or SMP timer queue.
