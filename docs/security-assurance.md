# Ginux defensive security assurance

This document records the security-review boundary established by HDAE Gates
32 through 37. The review is constructive and defensive: it inspects Ginux's
own source, runs controlled fixtures, and fixes weaknesses. It does not develop
exploits, build weaponized payloads, or test third-party systems.

## Trust and authority inventory

| Seam | Trusted owner | Required invariant | Primary evidence |
|---|---|---|---|
| Operator CLI | `ginux-cli` | Unprivileged parsing cannot directly mutate privileged state | CLI unit tests and helper protocol tests |
| Privileged lifecycle helper | `ginux-host-instance-helper.sh` | Instance names, paths, deployment state, and transitions are validated before host mutation | instance, lifecycle, and recovery suites |
| OCI acquisition | `ginux-oci` | Content is digest verified; archive paths and links cannot escape the selected root | OCI layer and cache-integrity tests |
| Contract compiler | `ginux-contract` and `ginux-instance` | Unknown syntax fails closed; compiled decisions are deterministic and digest bound | contract and configuration suites |
| Native supervisor | `ginux-init` | Contract and route policy precede host effect; copied caller state is not trusted twice | native supervision and security suites |
| Process authority | `ginux-process` | Virtual identity and descriptor generations cannot revive stale authority | process and multi-instance suites |
| Virtual kernel views | `ginux-kernel-views` | Instance-visible state is generated from private Ginux state, not host trees | kernel-view and systemd suites |
| Filesystem mediation | `ginux-fs-policy` after Gate 31 | Text and numeric descriptors are not authority; object identity and generations are required | Gate 31 host-effect oracles |
| Package lifecycle | RPM spec, tmpfiles, and helper | Code is immutable package content; persistent and transient state have explicit ownership and lifecycle | RPM install, verify, erase, reinstall, and persistence tests |
| Allocator | `ginux-allocator` after Gates 38-42 | Caller memory never contains allocator metadata; invalid ownership fails closed | allocator conformance, property, and Linux integration suites |

### Installed trusted executables

| Installed component | Privilege and host effects | Owner and invariant |
|---|---|---|
| `/usr/bin/ginux` | Operator process; requests lifecycle changes through `sudo` and the private helper protocol | `ginux-cli`; it validates public syntax but is not itself a privileged policy boundary |
| `/usr/libexec/ginux/ginux-helper` | Root lifecycle, instance-root, console, deployment-state, and cache operations | Lifecycle helper; names are validated, paths remain below owned roots, and stopped/running transitions are checked before mutation |
| `/usr/libexec/ginux/ginux-image` | Registry I/O, digest verification, blob caching, and root expansion | `ginux-oci`; verified content is committed atomically and extraction cannot escape its selected root |
| `/usr/libexec/ginux/ginux-runtime` | `chroot`, identity reduction, ptrace supervision, virtual-kernel generation, syscall mediation, and workload execution | `ginux-init`; the immutable effective contract and virtual object identity govern every managed effect |
| `/usr/lib64/ginux/libginux_allocator.so` | Optional workload-side virtual-memory calls and instance-private cgroup reads | `ginux-allocator`; it has no helper protocol or privileged path and remains subject to the workload contract |

Test-only fixtures and benchmark binaries are not installed and are outside the
runtime trusted computing base. The RPM file list is the authoritative package
boundary; the inventory checker rejects drift in these installed paths.

### Host-effect route matrix

| Route | Authority snapshot | Decision point | Effect owner |
|---|---|---|---|
| Instance create/import/set/edit | Complete candidate recipe plus deployment state | CLI validation, runtime normalization, then helper transition validation | Privileged lifecycle helper using atomic file replacement |
| OCI fetch and expansion | Digest-pinned manifest, layer descriptors, and selected destination root | Descriptor validation and content digest verification before commit | `ginux-image` cache and extraction code |
| Workload syscall | Entry registers plus one supervisor-owned copy of pointer-derived authority inputs | Immutable contract, operation catalog, dispatch lane, and object-generation checks | Native supervisor or an explicitly direct-safe host syscall |
| Descriptor and virtual-object operation | Instance-local descriptor/object identity and generation | Contract decision plus type-specific owner lookup | Process, device, network, filesystem, or virtual-kernel owner |
| Lifecycle stop/failure/recovery | Persisted instance identity plus verified live PID ownership | Helper state transition and stale-PID rejection | Helper cleanup of only that instance's transient resources |
| Allocator memory operation | Checked size/alignment and instance-visible memory limit | Allocator ownership bitmap and checked reservation | Ordinary workload `mmap`, `mprotect`, `madvise`, and `munmap`, still mediated by Ginux |

No managed route treats a text path, numeric PID, or descriptor number alone as
authority. Direct-safe syscalls are limited to cataloged fixed semantics whose
arguments do not require synchronous inspection, translation, virtual results,
or a denial-side host effect.

### Instance authority and revocation matrix

| Authority class | Instance-private identity | Revocation evidence |
|---|---|---|
| Processes and signals | Virtual PID plus generation mapped to one supervised host task | Process, native-process, multi-instance, and forced-teardown tests |
| Descriptors and devices | Per-process descriptor entry plus backing-object generation | Process/device suites, descriptor reuse tests, and restart tests |
| Root and writable storage | Validated instance directory and private expanded root | Multi-instance overlay checks and helper cleanup/reinstall tests |
| Contract and audit | Normalized recipe digest, catalog version, deployment lock state, and instance-owned log | Contract mutation, lock/unlock, multi-instance, and package persistence tests |
| Console | Instance-owned FIFO/log and verified supervisor identity | Attach/detach, signal, stale-state, and concurrent-console tests |
| Network | Instance deployment attachment, forward ownership, and tracked socket identity | Network policy, bind-conflict, destination-snapshot, and multi-instance tests |
| Virtual `/proc`, `/sys`, and cgroup | On-demand view derived from only the instance process/configuration state | Kernel-view, systemd, and distinct-memory-limit multi-instance tests |

Numeric identity reuse is never sufficient to recover an old capability. Stop,
failure, and restart discard transient generation-bearing state; persistent
recipe and root state survive only where the documented lifecycle says they do.

## Unsafe-code boundary

Production Rust `unsafe` is confined to the freestanding native runtime and
its syscall backends. Policy, configuration, virtual process, device, kernel
view, VFS, OCI, and public CLI libraries remain safe Rust. The runtime requires
raw startup pointers, register access, inline host syscalls, ptrace memory
transfer, and fixed supervisor-owned buffers; these are reviewed as trusted
seams rather than described as automatically safe because they are written in
Rust.

`scripts/check-security-inventory.sh` rejects new production unsafe syntax
outside the approved runtime files. It also checks the retained privilege and
package boundary markers. This is a structural tripwire, not a proof that each
unsafe expression is correct.

The approved unsafe/FFI/assembly owners are:

| Source | Reason it is trusted |
|---|---|
| `ginux-init/src/main.rs`, `launch.rs`, `native_clone.rs`, `native_network.rs`, and `supervisor.rs` | Freestanding entry, raw Linux syscalls, ptrace register and tracee-memory transfer, and fixed-buffer supervision |
| `ginux-allocator/src/lib.rs` | C ABI exports, raw allocation pointers, atomics, and Linux virtual-memory calls |
| `baseline-runner` and `process-native-fixture` | Test/benchmark-only raw syscall fixtures; not installed runtime components |

The x86-64 startup assembly is owned by `ginux-init/src/main.rs`; no separate
production assembly object is installed. All other production policy and state
libraries are required to remain free of Rust `unsafe` syntax by the inventory
tripwire.

## Dispatch and host effects

The immutable effective contract is authoritative. Selective dispatch may
continue only an explicitly allowed operation whose pinned catalog semantics
say it needs no synchronous inspection, translation, virtual result, or audit.
Paranoid dispatch has no direct path. A denial or complex operation is decided
by Ginux before the requested host syscall. Mutable caller structures that
affect authority must be copied once into supervisor-owned state.

Gate 31 separately owns the filesystem call-context and capability foundation.
Its result is referenced here and is not generalized to operation families that
have not passed their own host-effect oracles.

## Host privilege and installed state

Root-launched workloads set `PR_SET_NO_NEW_PRIVS`, drop to host UID/GID 65534,
and retain only Ginux virtual credentials. The helper remains privileged and is
part of the trusted computing base. Persistent instance and OCI state lives
under `/var/lib/ginux`; transient control state lives under `/run/ginux`.
Package verification and state-preserving reinstall tests are mandatory.

| State | Owner/mode boundary | Lifecycle |
|---|---|---|
| `/etc/ginux/examples` | RPM-owned, read-only example recipes | Replaced only by package lifecycle rules |
| `/var/lib/ginux/cache/oci` | Root-owned verified content-addressed cache | Preserved across package erase; corrupt content is rejected and may be reacquired |
| `/var/lib/ginux/instances/NAME` | Root-owned recipe, deployment state, private root, console, and audit data | Created/replaced atomically; retained across package reinstall; explicit instance removal owns deletion |
| `/run/ginux/instances/NAME` | Root-owned transient control state | Recreated at boot/start and revoked on stop or recovery |

The helper accepts only its versioned private protocol. Operator environment
values are not promoted into privileged paths without command-specific parsing;
workloads do not inherit helper descriptors, ambient capabilities, or root host
identity.

OCI manifests and layers are verified before use. Cache corruption fails
closed. Catalog changes do not silently recompile persisted wildcard contracts,
because doing so could change their meaning.

## Residual limitations

- The balanced backend trusts the host kernel's ELF loading, virtual memory,
  scheduling, signal, ptrace, and page-fault behavior.
- Ginux is not a hardware isolation boundary and is not VM-equivalent.
- The native runtime has a large manually reviewed unsafe surface. Structural
  inventory, fuzzing, and regression tests reduce risk but do not constitute a
  complete formal proof.
- Shared-host networking intentionally shares normal host routing and bind
  conflicts. Ginux mediates configured operations but does not claim a private
  network namespace.
- Formal assurance is component-specific. Proof or model-checking evidence
  applies only to the named functions, state transitions, and assumptions.
- No review can establish the absence of every defect. Security claims remain
  limited to the documented invariants and passing evidence.

## Review discipline

Potentially sensitive findings are fixed before actionable detail is placed in
public gate records. Every finding must end in a regression, an explicitly
accepted limitation, or a separately approved follow-up gate. A clean review
never replaces the complete build, package, integration, recovery, and
multi-instance test matrix.
