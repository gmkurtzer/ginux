# Linux ELF and Runtime Compatibility Tests

Ginux treats Linux executable loading and process startup as a tested compatibility
contract. Run the contract with:

```sh
make test-linux-runtime
```

The target boots the verification kernel in QEMU and fails unless every category
below passes in ring 3. It is also part of the normal `make test` gate.

| Category | Coverage |
| --- | --- |
| ELF64 executable forms | Static `ET_EXEC`, position-independent `ET_DYN`, multi-segment mappings, and entry metadata |
| PIE and process entry | PIE execution, initial stack, arguments/environment, and auxiliary vectors |
| Dynamic loader | `PT_INTERP`, loader-first entry, dynamic tags, dependencies, and dependency authority |
| Libraries and symbols | `RELATIVE`, `GLOB_DAT`, and `JUMP_SLOT` relocations plus cross-object symbol resolution |
| TLS and initialization | Static TLS layout, TLS relocations, pre-initializers, constructors, and initialization order |
| Heap and process break | `brk`, heap copy bounds, growth, and reset across `exec` |
| Virtual memory | Anonymous `mmap`, protection and range splitting, demand allocation, and reclamation |
| ELF hardening | Non-executable stack, RELRO, W^X, and malformed/overlapping ELF rejection |
| Exec lifecycle | Atomic image replacement and retirement of old image, mapping, stack, heap, and TLS state |

The fixtures are assembled and linked with Clang/LLD by `kernel/build.rs`. Build-time
checks validate their ELF class, machine, type, required program headers, load-segment
permissions, and stack policy before they are embedded. Runtime category checks live
in `tests/qemu/check-linux-runtime.sh`; the detailed individual proofs remain in
`tests/qemu/boot.sh`.

Real Alpine/musl and Rocky/glibc images add integration coverage through
`make test-alpine`, `make test-rocky-shell`, and `make test-rocky-dnf`.
