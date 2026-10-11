# Native Executable Container v0

**Status:** Development container parsed and hardware-booted.

The signed control-plane bundle is a bounded little-endian container. Its
canonical header is:

| Offset | Size | Field |
|---:|---:|---|
| 0 | 8 | `GINXBIN\0` magic |
| 8 | 2 | version (`1`) |
| 10 | 2 | header size (`32`) |
| 12 | 4 | exact total size |
| 16 | 4 | entry offset within code |
| 20 | 4 | exact code size |
| 24 | 4 | flags, currently zero |
| 28 | 4 | reserved, must be zero |
| 32 | variable | code bytes |

Code contains 1 through 4096 bytes, the entry offset must identify a byte in
that range, and the exact total size must be `32 + code_size`. No trailing data
is accepted. The boot manifest records the size and SHA-256 digest of the whole
container, not only its code.

The kernel authenticates the bundle before parsing this inner format. It then
copies only the code bytes into each non-writable executable mapping and enters
at the checked offset. Every QEMU run also requires rejection of truncated,
bad-magic, wrong-version, wrong-header-size, wrong-total-size, invalid-entry,
zero-code-size, nonzero-flags, and nonzero-reserved variants.

The current fixed proof payload also contains kernel-known service and restart
offsets within the same authenticated code segment. They are bounds-checked but
are not described by version 1's header, so this is not a general multi-entry
loading contract. A future format version must name every permitted entry.

Version 1 is deliberately position-fixed and single-segment. It has no
relocations, metadata, interpreter, imports, writable data segment, stack
request, architecture tag, or feature negotiation. Those require a new format
version rather than reinterpretation of reserved fields.
