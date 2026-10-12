# Native Service Bootstrap ABI v0

**Status:** Shared Rust definition, C binding, and compiled volume-service use
are verified by the clean build and QEMU gate.

## Entry contract

The x86-64 kernel enters a measured native service at its authenticated static
PIE entrypoint in ring 3. `RBX` contains the user virtual address of a
read-only, NX `ginux_native_service_boot_v0` record. The initial stack is
16-byte aligned, writable, NX, bounded to two pages, and isolated from the
image and bootstrap record by absent guard pages. All other register contents
are outside this version's contract.

The record is little-endian, C-aligned, exactly 64 bytes, and contains only
fixed-width integers:

| Offset | Width | Field |
|---:|---:|---|
| 0 | 8 | magic `0x30425358554e4947` (`GINUXSB0` in memory) |
| 8 | 2 | version `0` |
| 10 | 2 | structure size `64` |
| 12 | 4 | exact feature bitmap |
| 16 | 8 | service virtual address of virtio common configuration |
| 24 | 8 | service virtual address of the queue notification register |
| 32 | 8 | service virtual address of the owned DMA window |
| 40 | 8 | device-visible address of that DMA window |
| 48 | 8 | opaque capability handle for the delegated device object |
| 56 | 8 | reserved; must be zero |

Feature bit zero is `VIRTIO_BLOCK`. ABI v0 accepts exactly that one bit. The
kernel and service reject a wrong magic, version, size, feature bitmap, zero
resource field, or nonzero reserved field. Unknown extensions therefore fail
closed rather than being silently misinterpreted.

The virtual addresses name only mappings in the receiving service's address
space. The capability is opaque and conveys no authority unless it resolves in
that exact domain generation's capability table. The device-visible DMA
address is not a kernel pointer; in the initial QEMU profile it equals a
physical address, while a future IOMMU profile may supply an I/O virtual
address under the same field semantics.

## Sources and compatibility

`ginux-native-abi` is the canonical `no_std` Rust definition and contains
compile-time size/alignment assertions plus layout and malformed-header tests.
`native-service-bootstrap-v0.h` is the language-neutral C binding. Both are
release contract artifacts; neither exposes Rust layout, kernel pointers, page
table objects, interrupt-controller state, or internal domain records.

ABI v0 is exact-version only. A later compatible extension must define a new
accepted version and size policy before changing this structure. Recompiling a
v0 service is not required while the kernel continues to advertise support for
v0.
