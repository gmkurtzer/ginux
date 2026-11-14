# Ginux guarded allocator

Ginux includes an experimental workload-side allocator. It is a standard C ABI
shared library, not part of the syscall dispatcher. Programs use it through
normal dynamic linking or `LD_PRELOAD`; its `mmap`, `mprotect`, `munmap`, and
virtual cgroup reads remain subject to the instance contract.

The component is an independent Ginux implementation. Upstream challoc is a
design and comparison control, not vendored code and not a source of inherited
assurance claims.

## Enable it

The RPM installs the library at:

```text
/usr/lib64/ginux/libginux_allocator.so
```

Instance preparation copies it into the matching path inside the instance.
Add the following entry to the recipe's existing `instance.environment` list:

```toml
environment = [
  "container=ginux",
  "LD_PRELOAD=/usr/lib64/ginux/libginux_allocator.so",
]
```

The allocator is opt-in. Recipes without `LD_PRELOAD` retain the distribution's
normal allocator.

## Implemented ABI

The initial ABI includes `malloc`, `free`, `calloc`, `realloc`,
`reallocarray`, `posix_memalign`, `aligned_alloc`, `memalign`, `valloc`,
`pvalloc`, `cfree`, and `malloc_usable_size`. It also exports the common
jemalloc extended allocation entry points. Unsupported `mallctl` names return
`ENOENT` instead of pretending to change state.

## Design

- Allocator metadata lives in a fixed library-owned registry rather than in or
  next to caller allocations.
- Every allocation receives inaccessible guard pages on both sides of its
  writable mapping.
- Placement inside the guarded region is randomized when `getrandom` is
  available and uses a deterministic non-secret fallback otherwise.
- Size and alignment arithmetic is checked before mapping.
- Unknown, invalid, and duplicate frees terminate the workload deterministically
  without mutating another allocation.
- A process-aware spin lock resets inherited ownership after `fork`; allocation
  and release remain serialized across threads.
- Freed mappings are immediately returned to the kernel. The implementation has
  no retained slab or thread cache in this first version.
- `/sys/fs/cgroup/memory.max` is read through the instance's virtual kernel view.
  The allocator refuses a request that would exceed the visible limit according
  to its live requested-byte accounting.

## Assurance boundary

The implementation makes allocator metadata less reachable from ordinary
caller overflows, but it cannot make an unsafe application memory-safe. The
registry, raw pointers, C ABI, system calls, loader, host virtual-memory system,
and caller compliance remain trusted seams. Ginux does not inherit challoc's
published formal proofs.

The defensive suite covers ABI behavior, checked arithmetic, alignment,
multithreaded allocation, reallocation, fork, invalid release, packaging, and
matched native versus guarded throughput. Run it on Linux x86-64 with:

```sh
make test-allocator-linux
```

Raw benchmark rows are written to
`.artifacts/allocator/benchmark.tsv`. The guarded design favors simple authority
and immediate reclamation over allocator throughput; the report must retain
failed or slow rows rather than imply parity with optimized slab allocators.
