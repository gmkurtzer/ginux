# Gate 3 execution kernel

Gate 3 establishes the security-focused execution baseline. Ginus parses a
bounded static ELF64 x86-64 file into a fixed virtual address space represented
entirely by Ginus-owned bytes and page-permission metadata. It builds the Linux
x86-64 initial stack and interprets the fixture's machine instructions.

A managed `syscall` instruction is an interpreter event. Ginus sends the
request through the central dispatcher and records an audit decision. It never
executes the equivalent host syscall on behalf of the managed program. Managed
addresses are validated offsets into the Ginus address space and are never
dereferenced as host pointers.

The approved fixture uses the ordinary Linux x86-64 syscall ABI to write its
argument and environment value, request an unknown syscall, obtain its virtual
PID, and exit. The interpreter implements only the instruction subset required
by this bounded fixture profile. Dynamic linking, filesystems, threads, and
systemd are later gates.

Run the human checkpoint with:

```sh
make demo-gate-3
```

The demo prints the ELF, stack, memory, syscall, output, audit, and malformed
input evidence individually and states the compatibility work it does not yet
cover.
