# Reliable Protocol Egress v0

**Status:** A claimed protocol packet survives a faulted network-service
generation and is transmitted by its replacement under QEMU/TCG.

The protocol service publishes one exact ICMPv4 echo reply into object 9 and
retires. A network bootstrap v3 generation receives egress `READ`, claims the
sequence into its confined DMA page, proves a second claim is denied, and
proves acknowledgement is denied before hardware completion. The test-only
fault feature then drives the compiled driver's normal fatal instruction while
its virtio queues, bus mastering, MSI-X route, and VT-d domain are active.

Fault containment masks and disables MSI-X, revokes bus mastering, resets the
NIC, retires the interrupt route, disables and scrubs VT-d, scrubs DMA, revokes
the generation's capabilities, and releases its claim. The copied egress
packet remains unchanged at the queue head.

A fresh network generation receives new authority and leases, claims the same
sequence, publishes the exact virtio-net descriptor, and waits for kernel-owned
MSI-X completion. Only after the kernel independently validates the completed
descriptor and exact packet can operation 27 acknowledge destructive delivery.
Replay is then denied and normal ordered teardown retires the replacement.

This provides at-least-once retry across failure before acknowledgement. It
does not prevent a duplicate on an external network if hardware transmitted a
packet but the driver or kernel failed before recording completion. End-to-end
deduplication, peer acknowledgement, and exactly-once network delivery are not
claimed.
