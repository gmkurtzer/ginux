# Defensive security review record

This record closes the bounded source review in HDAE Gates 32 through 37. It is
a review of project-owned Ginux code and controlled fixtures. It contains no
exploit development, weaponized payload, or testing of third-party systems.

## Finding ledger

| ID | Area | Disposition | Permanent evidence |
|---|---|---|---|
| SR-01 | Trusted and unsafe seams could grow without an ownership tripwire | Fixed: the approved unsafe-file inventory and privilege/package markers now fail `make check` when they drift | `scripts/check-security-inventory.sh` and `docs/security-assurance.md` |
| SR-02 | The allocator package could install without executing its C ABI fixture | Fixed: RPM `%check` compiles and runs the conformance fixture through the packaged library build | `packaging/rpm/ginux.spec.in` and RPM tests |
| SR-03 | The first allocator design serialized and mapped every allocation, creating unnecessary availability risk under ordinary load | Fixed: bounded sharded arenas, atomic ownership bitmaps, and guarded large mappings replace the per-allocation mapping path | allocator unit, C conformance, stress, and matched benchmark suites |
| SR-04 | A reclaimed slab slot could become reusable before page reclamation completed | Fixed: pressure reclamation completes while the ownership bit remains live | allocator concurrent stress and source invariant |
| SR-05 | Global memory accounting used add-then-rollback and could transiently exceed its visible bound | Fixed: checked compare-and-exchange reservation never publishes an overflow or over-limit value | exhaustive bounded accounting model test |
| SR-06 | Accepted local sockets lost the listener's protocol classification, so legitimate replies could be denied after acceptance | Fixed: accepted descriptors inherit the managed listener protocol before any send decision | complete Rocky systemd startup matrix |
| SR-07 | A `waitid(..., WNOWAIT)` observation retired virtual child identity before the later reap | Fixed: peeking preserves the generation-safe child identity until a consuming wait | process and Rocky systemd lifecycle suites |
| SR-08 | Host-namespace clone attempts returned an ABI-absence result after systemd feature probing | Fixed: namespace flags are rejected before the host effect with `EPERM`, preserving the caller's safe fallback | process and Rocky systemd startup suites |
| SR-09 | Filesystem identity queries on instance-confined descriptors could expose host mount identity or fail valid startup probes | Fixed: confined descriptors return a stable virtual filesystem identity while proc and cgroup retain their specific types | kernel-view and Rocky systemd suites |
| SR-10 | The concurrent-instance regression still addressed transient control files through the persistent-state tree | Fixed: PID and console authority is read only from `/run/ginux`, and bounded failure logs make lifecycle regressions diagnosable | complete two-instance Rocky isolation workflow |

No unresolved high- or critical-impact finding remains within this review's
approved scope. That statement is not an independent audit or a claim that the
codebase has no defects.

## Review coverage

- Contract compilation, immutable digests, unknown-operation failure, dispatch
  selection, and audit ordering are exercised by the contract and security
  suites.
- Process, descriptor, device, virtual-kernel, network, root, console, and
  lifecycle authority are exercised by focused and multi-instance suites.
- Host identity reduction, `NoNewPrivs`, helper boundaries, OCI verification,
  package ownership, reinstall, retained state, and recovery are exercised by
  lifecycle, OCI, RPM, and recovery suites.
- Parser and lifecycle robustness use the bounded checked-in fuzz corpus. The
  allocator adds exhaustive arithmetic/class models and concurrent C stress.

## Assurance limitation

An external formal-verification tool was not forced onto the freestanding
runtime or allocator because no reviewed model could soundly cover raw Linux
syscalls, ptrace, the dynamic loader, and concurrent caller behavior as one
system. The reproducible substitute is bounded exhaustive modeling of pure
allocator invariants, deterministic property tests in the contract/runtime
crates, controlled fuzzing, and end-to-end QEMU evidence. These tests are useful
assurance, not a formal proof of the whole runtime.

The residual trust and product limitations remain those in
[`security-assurance.md`](security-assurance.md): the host kernel and loader are
trusted, the native runtime has manually reviewed unsafe seams, shared-host
networking is intentionally shared, and Ginux is not VM-equivalent.
