# ADR 0001: Native Domains and Boot-Bounded Authority

- **Status:** Accepted
- **Date:** 2026-10-10

## Context

Ginux needs a small immutable host, isolated and restartable system services, Linux-compatible containers, and a native control plane that can manage every workload without becoming able to rewrite the kernel or manufacture new privilege.

Process ancestry is useful for lifecycle and accounting but is not a safe authority mechanism. UID 0 is required inside Linux containers but must not imply native host authority. Third-party drivers and services must be replaceable without executing in the kernel address space.

## Decision

The kernel's native isolation and ownership primitive is a **domain**. A domain owns:

- One or more processes and address spaces.
- A capability table.
- CPU and memory budgets.
- IPC endpoints and shared-memory grants.
- Device, network, and filesystem grants.
- Failure, cleanup, and restart policy.

The authenticated boot manifest defines the maximum authority available for one boot. After initialization, the kernel creates exactly one initial userspace authority: the native control-plane domain. Every later domain and capability descends from authority explicitly delegated by an existing holder.

The kernel enforces these invariants:

1. Every runtime capability is authorized by the boot manifest or derived as a subset of an existing capability.
2. Delegation cannot increase rights, object scope, resource bounds, or lifetime.
3. Process ancestry establishes ownership, accounting, and default cleanup, but grants no authority by itself.
4. Revoked or destroyed handles cannot regain validity through identifier reuse; handles carry generations or an equivalent freshness mechanism.
5. No userspace component can modify kernel code, kernel read-only data, trust roots, or the active authority ceiling.
6. No third-party code executes in the kernel address space.
7. UID 0 is authoritative only within its Linux-compatible domain and has no implicit native-host rights.

Control-plane powers are distinct capabilities. Process enumeration, lifecycle control, resource assignment, memory debugging, device assignment, and capability delegation are not bundled into one ambient superuser privilege. Production policy may allow the control plane to stop every workload without allowing it to read every workload's memory.

## Consequences

- The control plane is highly trusted but remains constrained by kernel-enforced boot policy.
- Compromising the control plane may expose or destroy resources it was authorized to manage, but must not permit kernel modification or authority expansion.
- Native drivers, filesystems, networking, cryptography, and Linux compatibility run as isolated domains behind stable interfaces.
- Linux containers are domains with additional Linux ABI, namespace, credential, and virtual-filesystem behavior.
- Capability derivation, revocation, domain teardown, and resource cleanup become early specification and verification targets.
- Fast paths may use shared mappings or hardware queues only after a protected control path establishes the necessary grants.

## Initial validation

The first executable milestone boots through UEFI, fixes the authority envelope, creates one minimal domain, launches a control-plane process, and demonstrates that the process cannot invoke an operation absent from its boot-granted capabilities.

The bootstrap implementation records the generation-tagged control domain as
each service's lifecycle parent independently of per-capability delegation
provenance. Tests require ancestry to disappear with a retired generation and
to reattach explicitly for its replacement; neither relationship grants host
authority.
