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

The packaged shared library is currently a glibc x86-64 build. The package
matrix verifies loading with Rocky Linux 10 and Ubuntu 24 workloads, natively
through the host kernel and through Ginux. Alpine remains a supported Ginux
instance, but its musl userspace is not in the allocator compatibility claim;
attempting to preload this glibc library there is unsupported.

## Implemented ABI

The initial ABI includes `malloc`, `free`, `calloc`, `realloc`,
`reallocarray`, `posix_memalign`, `aligned_alloc`, `memalign`, `valloc`,
`pvalloc`, `cfree`, and `malloc_usable_size`. It also exports the common
jemalloc extended allocation entry points. Unsupported `mallctl` names return
`ENOENT` instead of pretending to change state.

## Design

- Allocator metadata lives in fixed library-owned registries and atomic slot
  bitmaps rather than in or next to caller allocations.
- Ordinary allocations use power-of-two size classes in per-thread-sharded
  1 MiB arenas. Each arena has inaccessible guard pages; large or unusually
  aligned allocations receive an individually guarded mapping.
- Arena placement and slot starting points are seeded from `getrandom` when it
  is available and use a deterministic non-secret fallback otherwise.
- Size and alignment arithmetic is checked before mapping.
- Unknown, invalid, and duplicate frees terminate the workload deterministically
  without mutating another allocation.
- A process-aware spin lock resets inherited ownership after `fork`. Ordinary
  slot allocation and cross-thread release use sharded atomic bitmaps; guarded
  mappings and arena initialization use the lock.
- Large freed mappings are immediately returned to the kernel. Arena pages are
  retained on normal fast paths and page-sized free slots are advised away when
  the instance reaches its configured pressure threshold.
- `/sys/fs/cgroup/memory.max` is read through the instance's virtual kernel view;
  the value is derived from that instance's configured `resources.memory_mib`,
  while `memory.current` is generated from only its supervised processes.
  Size classes acquire coarse credits against that hard limit, so fast-path
  accounting is local while aggregate reserved capacity cannot exceed the
  instance-visible ceiling.

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
`.artifacts/allocator/benchmark.tsv`, with five paired repetitions for each
thread count and separate provenance. The guarded design favors explicit
authority and metadata separation; reports retain failed or slow rows rather
than imply parity with mature optimized allocators.
