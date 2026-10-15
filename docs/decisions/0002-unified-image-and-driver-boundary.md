# ADR 0002: Unified Boot Image and Driver Boundary

- **Status:** Accepted
- **Date:** 2026-10-13

## Context

Ginux is not intended to host a small conventional Linux distribution beside its containers. The host needs only the kernel, boot-required hardware support, and a narrowly scoped controller for configuration, monitoring, maintenance, and container lifecycle. The design must also support normal systemd-based system containers, stable driver compatibility, runtime immutability, and failure containment.

The earlier architecture placed nearly all storage, networking, filesystem, and GPU support in userspace domains and rejected future binary Linux module compatibility. That no longer reflects the product direction.

## Decision

1. One authenticated `GINUX.EFI` packages the kernel, boot-required modules, controller payload, recovery support, and a versioned component catalog.
2. The controller is embedded as bytes but executes in ring 3 as a statically linked Linux-ABI process. It uses standard Linux syscalls for ordinary process I/O and lifecycle, plus a versioned Linux device/ioctl UAPI for Ginux-specific host management. It is not linked into the kernel's privilege domain and is the only initial userspace authority.
3. The native boot path has no separate initramfs or host root filesystem. Container-image formats, including CPIO fixtures, remain independent inputs.
4. Boot-critical storage, filesystem, networking, GPU, and primary-device support runs in the kernel or authenticated native kernel modules. Nonessential or experimental drivers may run in isolated userspace domains.
5. Native modules use a stable, versioned kABI. They are authenticated and selected before the runtime integrity seal; arbitrary post-seal insertion is forbidden.
6. Binary Linux `.ko` compatibility is deferred beyond the proof of concept, not rejected. Stable module-loading and device abstractions must preserve a future path to a bounded compatibility layer for selected authenticated modules.
7. Existing filesystem code and on-disk formats may be reused or adapted behind native module contracts; a new filesystem is not required merely because the kernel is new.
8. Authenticated local controller configuration declares autostart containers and init paths. Later mutually authenticated remote management remains subordinate to local sealed policy.
9. Rocky Linux 10 x86-64 booting systemd as PID 1 is the product compatibility benchmark. Minimal images are fixtures only.

## Consequences

- The privileged code base is larger than a strict microkernel, so module review, ABI conformance, W^X, signing, measurement, fault policy, and fuzzing become core release gates.
- The embedded controller is deployable as part of one image without receiving ring-0 privilege.
- Driver failures must be classified: isolated domains can restart; module failures must be contained where possible and must never silently corrupt kernel state. Graceful survival is an explicit design goal and requires subsystem-specific recovery contracts.
- The design is modular without relying on the ambiguous label “monolithic kernel.”
- Container compatibility work concentrates on Linux behavior observed inside a domain, including systemd and independent networking, rather than recreating a general-purpose host userspace.

## Supersedes

This ADR supersedes ADR 0001 only where ADR 0001 required all drivers, filesystems, and networking to run as isolated userspace domains or prohibited authenticated third-party kernel modules. Its authority and domain model remains accepted.
