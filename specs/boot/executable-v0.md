# Native Executable Container v0

**Status:** Development container parsed and hardware-booted.

The signed control-plane bundle is a bounded little-endian container. Its
canonical header is:

| Offset | Size | Field |
|---:|---:|---|
| 0 | 8 | `GINXBIN\0` magic |
| 8 | 2 | version (`2`) |
| 10 | 2 | header size (`40`) |
| 12 | 4 | exact total size |
| 16 | 4 | exact code size |
| 20 | 2 | entry count (`3`) |
| 22 | 2 | flags, currently zero |
| 24 | 4 | control entry offset within code |
| 28 | 4 | service entry offset within code |
| 32 | 4 | service-restart entry offset within code |
| 36 | 4 | reserved, must be zero |
| 40 | variable | code bytes |

Code contains 1 through 4096 bytes, all three entry offsets must be distinct
bytes in that range, and the exact total size must be `40 + code_size`. No
trailing data is accepted. The boot manifest records the size and SHA-256
digest of the whole container, including its entry metadata.

The kernel authenticates the bundle before parsing this inner format. It then
copies only the code bytes into each non-writable executable mapping and enters
each role at its authenticated offset. Every QEMU run also requires rejection of truncated,
bad-magic, wrong-version, wrong-header-size, wrong-total-size, invalid-entry,
duplicate-entry, wrong-entry-count, zero-code-size, nonzero-flags, and
nonzero-reserved variants.

Version 2 is deliberately position-fixed and single-segment. It has no
relocations, metadata, interpreter, imports, writable data segment, stack
request, architecture tag, or feature negotiation. Those require a new format
version rather than reinterpretation of reserved fields.
