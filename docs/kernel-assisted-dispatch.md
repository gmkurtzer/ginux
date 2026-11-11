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
| loadable module with ftrace IP modification | Potentially, by redirecting selected syscall implementations | A module can return an errno; userspace emulation needs a separate queue | Requires module-owned task identity and fork/exec/exit tracking | Only hooked syscall implementations need pay | Prototype; compatibility and safety risks must be measured |
| kprobe execution redirection | Technically possible by changing registers and instruction pointer | Module-specific | Requires module-owned task identity | Only probed functions need pay | Diagnostic fallback only; kernel documentation warns of architectural and crash risk |
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

## Prototype questions

The kernel-module prototype must answer these questions before it can become a
supported backend:

- Are the target syscall entry functions present and ftrace-modifiable on the
  supported Rocky kernel without colliding with livepatch?
- Can an instance be attached while its initial task is stopped, then inherit a
  stable policy identity across every fork, clone, exec, and exit without an
  uncontrolled interval?
- Can policy lookup and selected denial remain RCU-safe without allocations,
  sleeping, or userspace dependency in the ftrace callback?
- Can module unload be refused while policies or managed tasks exist, and can
  supervisor loss force managed tasks into a fail-closed state?
- Does the measured improvement justify an out-of-tree module's kernel-version,
  signing, packaging, and incident-response burden?

## Evidence

Pending Gate 16 prototypes and measurements.
