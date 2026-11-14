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
