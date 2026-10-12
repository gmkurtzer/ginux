# Virtio Network Masked MSI-X v0

**Status:** Masked programming is verified on QEMU Q35.

## Purpose

This contract prepares distinct receive and transmit interrupt messages without
making either message deliverable and without granting DMA authority.

## Programming

The kernel requires the virtio-net function to begin reset with PCI I/O,
memory, and bus-master bits clear and MSI-X globally disabled. Because the
table resides in a memory BAR, it temporarily enables PCI memory decoding only.
Bus mastering remains clear.

Table entry 0 is assigned local-APIC vector 34 for receive completion and table
entry 1 is assigned vector 35 for transmit completion. Each entry contains the
current APIC destination, zero upper address, its exact vector, and the per-entry
mask bit. Full readback is required while the global MSI-X enable bit remains
clear. PCI memory decoding is then revoked again.

## Rejection coverage

Before programming, the kernel snapshots both entries and attempts an
out-of-range table index, an exception vector, and the syscall vector. Every
request must fail without changing either entry. A table shorter than two
complete entries also fails closed.

## Deliberate limits

- No IDT gate is installed and MSI-X is never enabled in this milestone.
- No queue is bound to a vector and no descriptor is published.
- The next milestone must mask globally while installing gates and queue-vector
  bindings, then unmask only after VT-d and queue state are live.
