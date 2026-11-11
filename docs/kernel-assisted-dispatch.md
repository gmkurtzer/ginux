# Kernel-assisted syscall control research

This document records Gate 16 research and prototype evidence. It distinguishes
mechanisms that can enforce an instance contract before a host effect from
mechanisms that can only observe an operation or emulate it after a userspace
round trip.

## Required semantics

An enforcement backend is usable only when it provides every semantic required
by the compiled instance contract:

1. deny before the host operation has an effect;
2. cover raw syscalls, static binaries, threads, fork, clone, and exec;
3. bind policy to an instance without a task-visible bypass;
4. replace policy atomically and fail closed on stale identity or supervisor
   loss;
5. retain the existing Ginux userspace path for operations whose result or
   visible kernel state must be emulated or translated; and
6. avoid charging unrelated allowed syscalls for policy they do not need.

Portable instance recipes state these semantics. They do not select an
implementation backend.

The machine-checked operation-class inventory is
[`kernel-assisted-requirements.tsv`](kernel-assisted-requirements.tsv). It is
validated by `make test-gate16-requirements` so a class cannot silently lose
its pre-effect requirement, mechanism, fallback, or disposition.

## Mechanism matrix

| Mechanism | Pre-effect deny | Emulate or translate | Scope and inheritance | Allowed-call cost | Gate 16 disposition |
|---|---|---|---|---|---|
| seccomp cBPF `ERRNO`/`KILL` | Yes, by syscall number and scalar arguments | No | Per task; inherited across fork/clone and exec | Filter path applies to every syscall, with kernel allow caching where possible | Retained fallback for syscall-number policy |
| seccomp user notification | Yes while the notification remains pending | Yes, with explicit userspace response | Inherited filter; listener lifecycle must be managed | Userspace round trip for notified calls | Retained for operations requiring userspace decisions |
| seccomp `TRACE` plus ptrace | Yes while stopped | Yes, including register and memory handling | Ptrace process tree plus inherited filter | Ptrace stop for selected calls | Current compatibility fallback |
| BPF LSM | Yes at supported security hooks | No ABI-result emulation | System-wide attachment; maps may scope policy by cgroup or task identity | Only relevant LSM hooks execute | Prototype for filesystem, credentials, executable, and other object policy |
| cgroup BPF | Yes at supported device, network, socket, and sysctl hooks | No general ABI emulation | Cgroup membership naturally follows tasks and descendants | Only relevant cgroup hooks execute | Prototype/complement for object-specific policy |
| Landlock | Yes for supported filesystem, network, capability, namespace, and scope rights | No | Applied by a task and inherited; restrictions can only tighten | Only relevant LSM hooks execute | Supported-kernel complement; not a complete dynamic contract backend |
| namespaces, cgroups, rlimits, mount topology | Yes for their native resource boundaries | Native kernel view rather than arbitrary translation | Kernel-managed task hierarchy | No general syscall filter | Preferred whenever contract semantics map exactly |
| Syscall User Dispatch | Signal before selected syscall execution | Userspace signal handler can emulate | Per thread; resets across fork/exec and is controlled by task memory | Direct region is fast; dispatched calls pay signal cost | Rejected for security enforcement; optional compatibility research only |
| tracepoint/fentry/fexit BPF | Primarily observation | No general safe denial | Attachment-dependent | Hooked event cost | Rejected as sole enforcement path |
| BPF `fmod_ret` | Only on kernel functions explicitly permitting return modification | Limited return replacement | Attachment-dependent | Hooked function cost | Insufficient for arbitrary syscalls |
| loadable module with ftrace IP modification | Potentially, by redirecting selected syscall implementations | A module can return an errno; userspace emulation needs a separate queue | Requires module-owned policy identity | Only hooked syscall implementations need pay | Rejected on the tested Rocky kernel: required syscall entry symbols are not ftrace-modifiable |
| kprobe execution redirection | Yes in the bounded prototype by replacing the selected syscall entry IP | Module-specific | Prototype scopes policy by cgroup, which survives thread/fork/exec | Only probed functions need pay | Implemented experimentally; kernel documentation warns of architectural, versioning, and crash risk |
| syscall-table patching | Technically possible on some builds | Module-specific | Global, unsupported | Potentially low | Rejected: unsupported internal patching, global conflict, and unsafe lifecycle |
| libc or binary rewriting | No reliable coverage | Limited | Bypassable by raw syscalls and static code | Low | Rejected as enforcement |

## Primary Linux references

- [Seccomp BPF](https://docs.kernel.org/userspace-api/seccomp_filter.html)
  defines kernel-resident deny actions, inheritance, ptrace notification, and
  userspace notification. It also documents that pointer memory cannot be
  dereferenced by the filter and that seccomp is one sandbox building block,
  not a complete sandbox.
- [BPF LSM programs](https://docs.kernel.org/bpf/prog_lsm.html) can attach at
  runtime to supported LSM hooks and return an error before an operation.
- [Linux Security Module usage](https://docs.kernel.org/admin-guide/LSM/index.html)
  clarifies that conventional LSMs are boot-selected kernel extensions, not
  ordinary loadable modules.
- [Syscall User Dispatch](https://docs.kernel.org/admin-guide/syscall-user-dispatch.html)
  explicitly states that it is not a security mechanism, is task-bypassable,
  and resets across fork and exec.
- [Using ftrace to hook functions](https://docs.kernel.org/trace/ftrace-uses.html)
  documents `FTRACE_OPS_FL_IPMODIFY`, its use for function redirection, its
  single-owner restriction, recursion hazards, and permanent-registration
  behavior.
- [Kernel probes](https://docs.kernel.org/trace/kprobes.html) documents that a
  pre-handler can change registers and the instruction pointer, while warning
  that execution-path changes can easily crash a kernel.
- [Landlock userspace API](https://docs.kernel.org/userspace-api/landlock.html)
  defines inherited stackable restrictions and their filesystem, network, and
  current ABI limitations.

## Automatic backend planning

The planner consumes normalized contract requirements and host capabilities.
It emits a set of enforcement components rather than one recipe-selected
dispatcher:

1. map exact resource restrictions to native namespaces, cgroups, rlimits,
   mount topology, Landlock, BPF LSM, or cgroup BPF when available;
2. compile simple syscall-number denials to a kernel-resident decision path;
3. route only operations requiring Ginux emulation, pointer-aware inspection,
   translation, or virtual kernel state to userspace;
4. retain ptrace/seccomp as the sound fallback for requirements a faster path
   cannot satisfy; and
5. reject startup when no available combination satisfies the complete
   contract.

The runtime reports the selected components and why each was required. A debug
override may force a slower sound implementation, but cannot force an
implementation that weakens the contract.

The checked-in `ginux-instance` planner implements this as a deterministic
composition rather than a single backend enum. Supported kernel APIs are
preferred; the experimental module requires a host-side opt-in. Any uncovered
requirement either selects userspace mediation or fails startup. Unit tests
cover mixed component selection, userspace-only semantics, module opt-in, and
an unsatisfied fail-closed plan.

## Prototype answers

- `__x64_sys_mkdirat` is kprobe-visible on Rocky
  `6.12.0-211.16.1.el10_2.0.1.x86_64`, but it is the only tested syscall entry
  among `getpid`, `sched_yield`, `mkdirat`, and `do_syscall_64` exposed through
  the ftrace filter. The module prototype therefore uses a kprobe and does not
  claim a stable kernel ABI.
- A cgroup-bound policy covers the current task and descendants. Raw syscall,
  pthread, fork, and post-exec static fixtures are all denied before a
  filesystem mutation.
- Policy lookup is RCU-protected and performs no allocation or userspace round
  trip in the kprobe handler.
- Policy survives controller exit. The module holds its own reference while a
  policy is active, refuses unload, and can be unloaded only after explicit
  policy removal.
- The prototype demonstrates feasibility, not production suitability. Every
  supported kernel build would need compilation, signing, symbol and behavior
  validation, upgrade coordination, and an incident-safe rollback path.

## Evidence

`make test-gate16-kernel-assist` builds both prototypes against the exact Rocky
kernel, loads them only in the disposable QEMU host, and verifies raw, thread,
fork, exec, detach, controller-loss, and unload behavior. Both paths pass. The
BPF LSM link and map remain pinned after the loader exits; removing the link
detaches enforcement deterministically. The host reports Landlock ABI 6.

The raw five-sample results and provenance are checked in under
[`benchmarks/gate16/results/`](../benchmarks/gate16/results/). Higher is better.

| Workload | Native/control mean ± sample SD | Module mean ± SD | BPF LSM mean ± SD |
|---|---:|---:|---:|
| Unrelated `sched_yield` calls/s | 785,041 ± 13,167 | 783,282 ± 9,869 (0.998x) | 789,950 ± 1,839 (1.006x) |
| Allowed `mkdirat` create/remove pairs/s | 7,475 ± 56 | 7,475 ± 82 (1.000x) | 7,480 ± 96 (1.001x) |

The differences are within the variance expected from TCG and show the key
result: unrelated allowed syscalls do not enter a userspace dispatcher. For a
denied `mkdirat`, seccomp produced 2,253,167 ± 29,922 denials/s, the module
1,614,034 ± 34,374 denials/s, and BPF LSM 323,267 ± 4,001 denials/s. These
figures are not equivalent hook points: seccomp rejects at syscall entry,
whereas BPF LSM performs object-aware policy at the VFS security hook.

Gate 15 remains the authoritative current-runtime comparison. Its matched
matrix shows why kernel assistance matters: selective is close to native for
direct-safe operations, while paranoid all-syscall userspace notification is
orders of magnitude slower on thread-yield. Gate 16 does not relabel prototype
microbenchmarks as a full production runtime benchmark.

## Recommendation

Do not make the out-of-tree module the default. Keep it as an experimental
accelerator until kernel-version compatibility, packaging, signing, complete
operation coverage, policy update, and rollback are proven. Its kprobe entry
redirection is materially less stable than supported security APIs.

The preferred production path is a contract compiler that combines native
namespaces/cgroups/rlimits, BPF LSM, cgroup BPF, Landlock where its monotonic
allow-list model fits, and seccomp for simple syscall-number denial. Only ABI
emulation, translation, virtual `/proc` and `/sys`, and pointer-aware semantics
without a supported kernel hook remain in the userspace path. Ptrace/seccomp
notification stays as the sound compatibility fallback. Startup fails rather
than weakening the contract when the host cannot satisfy every requirement.
