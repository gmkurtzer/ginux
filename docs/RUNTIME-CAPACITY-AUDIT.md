# Runtime Capacity Audit

Ginux must not use compile-time collection sizes as accidental runtime quotas.
Runtime collections should allocate pages only when needed, grow transactionally,
and scrub and return empty pages promptly. Fixed sizes remain valid only when they
are part of an external ABI, a hardware format, or a deliberately documented
security or policy boundary.

## Current classification

| Area | Current behavior | Classification / next action |
|---|---|---|
| tmpfs file data | Page chain, allocated on write and released on unlink | Elastic |
| tmpfs file metadata | Page chain, allocated on create; empty pages are scrubbed and released | Elastic |
| overlay nodes, data, and xattrs | Page-backed chains | Elastic; per-page counts are packing factors, not global limits |
| OCI image payloads | Page-backed staging with domain accounting | Elastic payload; object/layer tables still need migration |
| local socket payloads | Page chain | Elastic payload; socket descriptor and TCP/UDP buffer tables still need migration |
| process address-space snapshots | Large per-process static mapping arrays | Highest-priority migration; requires owned, reference-stable snapshots across fork/exec/refresh |
| process, thread, capability, mount, and user-namespace tables | Static slot tables | Migrate to generation-safe page-backed slabs |
| domain and virtual-network inventories | Static slot tables | Migrate after the control ABI gains paginated enumeration and opaque handles |
| controller command line | Fixed native-console record | Migrate after the Linux `mmap`/`munmap` runtime path is itself elastic |
| packet queues and device rings | Fixed by negotiated queue geometry and MTU | Intentional hardware/protocol bound |
| page tables, page size, ELF/PE headers, digests, and volume sectors | Architecture or wire-format sizes | Intentional fixed representation |
| names and paths | Some ABI fields are fixed; internal 64/256-byte buffers remain | Preserve fixed wire records; add indirect storage for internal Linux limits |

## Rules for migrations

1. Allocation failure leaves the previously published state intact.
2. Growth uses checked arithmetic and available memory, not a hidden item-count ceiling.
3. Shrink, unlink, close, and exit scrub and release pages immediately when safe.
4. Handles remain generation checked; moving metadata must not invalidate a live borrowed snapshot.
5. Tests cross at least one page boundary and prove the kernel frame count returns to its starting value.
6. A fixed bound names its source: ABI, hardware, security policy, or temporary implementation debt. Raising an unexplained constant is not a fix.

## Known baseline blockers

The committed baseline independently fails the full QEMU verification at the
Linux process-lifecycle contract and fails `test-c1` at an outdated ELF
auxiliary-vector assertion. Both reproduce with this change reverted. `make
check` remains the reliable regression gate until those pre-existing failures
are repaired.
