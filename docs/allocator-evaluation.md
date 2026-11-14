# Allocator architecture evaluation

Ginux evaluated three ways to add a hardened workload allocator. The decision
is intentionally narrower than claiming to reproduce another project's
assurance or compatibility.

| Option | Compatibility | Assurance and trust | Performance and maintenance | Decision |
|---|---|---|---|---|
| Support upstream challoc | Upstream documents a C/glibc and common jemalloc ABI, but currently tests Chainguard/Wolfi with glibc 2.44 or later on x86-64 and AArch64 | Strongest upstream evidence, including published component proofs; Ginux would inherit only the binary behavior it independently tests | External release and build dependency; upstream reserves a large virtual-address layout | Supported as a future workload choice, not selected as Ginux's packaged default |
| Vendor or fork challoc | Could be adapted to Ginux distributions | Forking would require independently maintaining the proof/code relationship and clearly separating inherited from changed claims | Highest long-term maintenance burden | Rejected |
| Independent Ginux allocator | ABI and distribution support can be limited to what Ginux tests and packages | Small reviewable implementation; no inherited formal-assurance claims | Can consume Ginux's virtual memory limit directly and be optimized against matched controls | Selected as an opt-in experimental component |

The selected design is an ordinary workload-side shared library. It does not
join the syscall dispatcher or gain hidden authority. Its memory operations and
read of `/sys/fs/cgroup/memory.max` are governed by the same instance contract
as the workload.

## Upstream comparison boundary

The upstream challoc project is an architecture and comparison control. Its
documented properties informed metadata separation, guarded regions,
randomized placement, cgroup awareness, runtime probing, and explicit limits on
formal claims. Ginux does not vendor its source and does not claim its proof
coverage.

A digest-pinned upstream binary comparison was not treated as a valid Rocky 10
control because the upstream tested baseline requires a newer glibc than Rocky
10 provides. Substituting an unvalidated binary would produce a misleading
failure or compatibility claim. The retained matched controls are the native
distribution allocator, Rocky's packaged jemalloc, and the independently built
Ginux library in the same Rocky QEMU environment. A future challoc row requires
a supported upstream release for that environment or a separately reviewed
compatible build.

## Required interface

The initial Ginux library exports the selected standard C allocation ABI and a
bounded common jemalloc compatibility subset documented in
[`allocator.md`](allocator.md). It requires Linux `mmap`, `mprotect`, `munmap`,
`madvise`, `getrandom`, basic file reads, and the instance-private virtual
cgroup memory limit. Unsupported control-plane operations return a stable error
rather than pretending to change state.

The implementation remains opt-in through `LD_PRELOAD`. It is not a default
runtime dependency and cannot weaken an instance whose recipe does not enable
it.
