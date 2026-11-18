# Ginux HDAE Plan

## Strategy

Build Ginux from the security boundary outward. First prove that unmodified
x86-64 code can run without unmediated host-syscall effects. Then add a cached
Rocky userspace, instance configuration, Linux runtime behavior, virtual kernel
objects, systemd compatibility, explicit contract controls, performance
evidence, and boundary hardening.

Every implementation gate uses a dedicated `hdae/<gate-slug>` branch based on
`main` and is intended for its own pull request. Local commits must be signed
off. The human authorized pushing each dedicated gate branch and opening its
pull request on 2026-11-03. This does not authorize pushing directly to `main`,
merging, or closing a pull request.

The human authorized the twelve remaining gates (positions 1 through 11 and
13) as one ordered batch on 2026-11-03. Position 12 was removed at the human's
direction because a public-exploit reproduction is not required for this
constructive project. All twelve gates remain human checkpoints: after each
gate passes, update the evidence, push its dedicated branch, open or update its
pull request, run the human demonstration, and stop for acceptance before
advancing. Batch authorization does not permit advancing past an unaccepted
checkpoint, weakening criteria, expanding scope, or resolving a material
architecture decision silently.

On 2026-11-05 the human inserted and authorized corrective Gate 9.5 after
hands-on Gate 9 testing. It moves image acquisition into instance creation,
repairs shell job control and procfs compatibility, makes launch configuration
authoritative, adds persistent attach/detach, and cleans Ginux-caused systemd
diagnostics. The previously numbered instance-specification gate moved, while
still open, to position 9.75.

Every gate must provide a working human demonstration through
`make demo-gate-N`, where `N` is the gate position. The target must document
prerequisites, state what the human should observe, return nonzero when its
machine-checkable checks fail, and clean up or provide an explicit cleanup
target. A demonstration supplements automated acceptance evidence; it does not
replace it.

## Ordered gate map

1. **Rocky host foundation and interception proof**: boot a normal Rocky Linux
   10 host under QEMU TCG, run Ginux as an ordinary process, and prove matched
   syscall interception and benchmark controls without replacing host systemd.
2. **Rocky image-pipeline foundation**: reproducibly extract and verify Rocky
   filesystem content as pipeline evidence and fixtures. Gate 5 supersedes its
   cloud-image-derived instance root with the approved OCI instance-image path.
3. **Execution kernel**: address spaces, ELF loading, syscall ABI, central
   dispatch, and small unmodified programs.
4. **Instance configuration and lifecycle**: versioned configuration,
   normalization, create/start/inspect/stop, resources, and failure behavior.
5. **OCI-backed virtual VFS and native dynamic userspace**: explicit OCI
   acquisition, verified cached layers, virtual descriptors, immutable root and
   writable overlay, and a Rocky shell executed with the balanced profile under
   continuous ptrace syscall-entry/exit supervision.
6. **Device ownership and virtual hardware**: virtual `/dev`, console, block
   or bounded partition, virtual networking, exclusivity, revocation, and
   device-path benchmarks.
7. **Process runtime**: process, thread, signal, synchronization, time,
   credential, and event behavior required by systemd.
7.5. **Transitional project rename**: complete the first active-project rename,
   select `ginux` as the command, preserve locked historical lineage records,
   and rerun Gate 7 before human acceptance.
7.75. **Ginux final project rename**: rename active product and repository
   surfaces to Ginux while retaining the transitional checkout until the
   unified repository transition.
8. **Virtual kernel views**: Ginux-owned `/proc`, `/sys`, cgroup v2, mounts,
   namespaces, and machine identity without host leakage.
8.5. **Unified repository transition**: preserve the final standalone kernel
   as `bootable-kernel`, join its history to the accepted userspace-kernel
   lineage, and deliver the userspace tree through a pull request to Ginux
   `main`.
9. **Systemd baseline**: Rocky systemd as virtual PID 1 reaching a usable
   multi-user login, virtual PID and lockdown inspection, host-shared networking,
   a live upstream DNF update workflow, and controlled shutdown.
9.5. **Systemd usability and operator lifecycle**: create-time OCI acquisition,
   configuration-derived launch, Bash job control, Ginux-owned procfs,
   persistent attach/detach, and clean systemd compatibility diagnostics.
9.75. **Instance specification and network controls**: comprehensive versioned
   instance configuration, deterministic file/CLI precedence, interface and
   source-address selection, ingress and egress controls, port translation,
   volumes, access, identity, resources, devices, hooks, and effective-config
   evidence.
10. **Configurable contract and audit**: versioned syscall, ioctl, device,
    filesystem, and kernel-view policy with fail-closed validation and logs.
10.25. **Instance terminology migration**: cleanly replace the active `domain`
    product vocabulary with `instance` across CLI, configuration, code,
    persistence, audit, tests, and documentation without legacy aliases.
10.5. **Intuitive contract configuration and UX**: make the complete contract
    self-contained in one instance recipe using human access verbs, resource
    namespaces, bounded globs, deterministic deny precedence, and explanatory
    CLI workflows compiled into the Gate 10 enforcement table.
11. **Performance characterization and optimization**: native, conventional
    container, and Ginux comparisons against approved budgets.
12. **Contract and instance-network UX**: replace the active contract syntax
    with a hierarchical, self-contained recipe; compile bounded syscall globs;
    and move host-specific network attachment and forwarding to repeatable,
    atomic `instance create` and `instance set` flags.
13. **Boundary hardening**: malformed ABI tests, fuzzing, escape attempts,
    Ginux privilege reduction, recovery, performance regression, and explicit
    limitations.
15. **Dispatch-mode performance matrix**: reproducibly compare native Rocky,
    selective Ginux, and paranoid Ginux with paired controls, reusable test
    templates, combined tables and graphs, and complete operator documentation.
17. **Concurrent-instance isolation**: run multiple Rocky systemd instances in
    parallel and prove independent contracts, virtual state, writable overlays,
    resources, audit, console, lifecycle, recovery, and documented shared-host
    network behavior.
18. **Architecture explanation and differentiation**: answer recurring UML,
    gVisor, namespaces, VM, syscall-capture, security, compatibility, and HDAE
    questions with evidence-based README and architecture documentation.
19. **Readable instance recipes and hooks**: support self-contained multiline
    pre-boot scripts, remove the controller's Rocky-only OCI restriction, and
    provide validated Rocky, Ubuntu CMS, and Alpine native-boot recipes.
25. **Declarative operator CLI and internal command boundary**: inventory and
    re-evaluate every command, obtain human approval of the public taxonomy,
    replace hand-written public parsing with a Cobra-like Rust command
    framework, and keep runtime/backend commands out of the operator surface.
26. **Rocky Linux RPM packaging**: build one native Rocky Linux 10 x86-64 RPM
    and SRPM for the accepted public CLI, private runtime, helper, image tools,
    examples, and state layout; verify clean install, operation, upgrade,
    state-preserving erase, reinstall, and complete package metadata.
31. **Filesystem mediation foundation**: establish complete immutable syscall
    context, explicit operation ownership, typed root/cwd/descriptor authority,
    and independent host-effect oracles before the filesystem lane claims any
    syscall.

Gates are stored in `hdae/gates-todo/` and remain there until they pass and are
accepted at their checkpoint. Closed gate records move unchanged to
`hdae/gates-done/`. Intentionally postponed gates move to
`hdae/gates-deferred/` only with a recorded reason and return condition.

## Build and verification interface

Gate 1 establishes the top-level Makefile. Later gates add stable targets while
preserving these rules:

- `make test` is deterministic and requires no network access.
- Only an explicit acquisition target such as `make image-fetch` may download
  external content.
- Cached images and generated benchmark data stay out of Git unless a small,
  reviewed fixture or report is intentionally committed.
- Each target returns nonzero when mandatory verification fails.
- Aggregate targets never hide a failing mandatory subtarget.
- Benchmark controls run inside the same emulated x86-64 environment as Ginux
  unless a gate explicitly documents a non-comparable host measurement.

Planned public targets are:

```text
make doctor
make build
make check
make test
make test-unit
make test-intercept
make image-fetch
make image-verify
make image-prepare
make test-image
make test-runtime
make test-config
make test-instance
make test-vfs
make test-process
make test-devices
make test-block
make test-network
make test-kernel-views
make test-cgroup
make test-systemd
make test-contract
make test-security
make test-recovery
make bench-baseline
make bench
make bench-compare
make bench-block
make bench-network
make bench-report
make demo-gate-N
```

Targets become mandatory only in the gate that introduces them. Gate records
define their expected result.

## Traceability

- Kernel-exposure control is proven by Gates 1, 3, 10, and 13.
- General systemd compatibility is built by Gates 2, 3, 5, 7, 8, and 9.
- Per-instance configuration and ownership are built by Gates 4, 6, 8, 9.75,
  10, and 12.
- Device mediation is built and measured by Gates 6, 10, 11, and 13.
- Performance is measured from Gate 1 and judged in Gates 9, 11, and 13.
- Cross-instance authority and lifecycle isolation are proven by Gate 17.
- Operator CLI coherence, discoverability, and the public/internal command
  boundary are established by Gates 12.25 and 25.
- Reproducibility and offline testing are established by Gates 1 and 2 and
  remain regression criteria for every later gate.

## Deferred roadmap

After the initial twelve gates, Gate 14 preserves the Unicorn Engine execution
backend as a concrete alternative to native ptrace execution. It remains
explicitly deferred. A later kernel-module enforcement experiment was
abandoned on 2026-11-11 and is not part of the roadmap. Other likely
milestones include multiple concurrent
instances, conventional containers within an instance, network and storage backend
expansion, physical-device assignment with a separately reviewed IOMMU/VFIO
model, non-x86 architectures, live lifecycle controls, distribution expansion,
and production operational hardening. None is authorized by this plan.
