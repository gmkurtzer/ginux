# ADR 0003: Native Controller Desktop and Domain Presentation

- **Status:** Proposed for Gate 19 acceptance
- **Date:** 2026-10-31
- **Supersedes:** The blanket deferral of graphical desktops in the initial
  proof-of-concept scope; it does not supersede ADR 0001 or ADR 0002

## Context

Ginux currently boots an embedded Linux-ABI controller as the initial ring-3
authority and manages Linux-compatible domains through a serial command-line
interface. GitHub issue #12 demonstrated interest in graphical output, but its
implementation is not the architecture selected here.

The controller should be usable through either a CLI or a graphical interface.
It should also be able to present a complete Linux desktop without making that
desktop the host control plane, granting it permanent physical-device
ownership, or importing a Linux display server into the trusted host design.

Three boundaries must remain distinct:

1. kernel device and capability enforcement;
2. native controller composition and system management; and
3. Linux-domain desktop compatibility.

## Decision

### Native controller layer

Ginux will implement a native software compositor and window system as a
measured ring-3 controller service. The resident CLI controller remains the
initial management authority and recovery interface. It supervises the
graphical service as a distinct process/address space with attenuated display,
input, IPC, and management capabilities.

The graphical controller application is an ordinary native client. It and the
CLI invoke the same versioned management operations. No configuration or
authority exists only in the GUI.

Window policy, composition, widgets, focus, and presentation remain in ring 3.
The kernel supplies bounded devices, memory grants, IPC, scheduling, capability
checks, and cleanup; it does not become a GUI toolkit or window manager.

### Native display protocol

Native Ginux applications use a new versioned display protocol rather than
Wayland. The protocol has opaque generation-safe object handles, bounded
messages, capability-scoped connections, explicit buffer ownership, damage and
commit operations, frame completion, input events, and deterministic teardown.

Version zero is intentionally small. It is a Ginux contract and makes no claim
of Wayland wire compatibility. The detailed contract is in
[`docs/CONTROLLER-DESKTOP.md`](../CONTROLLER-DESKTOP.md).

### Physical seat ownership

The controller owns the physical display, keyboard, and pointer by default. A
seat records display focus, keyboard focus, pointer focus/capture, and active
leases. Input delivery depends on both focus and capability; receiving a device
event never creates authority.

Exclusive full-screen use is a temporary logical lease. It changes presentation
and input routing while the controller retains device ownership. A platform
secure-attention chord is handled before client routing and synchronously
returns presentation and input to trusted controller UI. Raw GPU and physical
input passthrough are deferred.

### Linux domains

A Linux domain receives a virtual display and virtual input endpoint. A small
compositor inside the domain exposes an ordinary Wayland socket to unmodified
selected Linux clients, composes their windows, and exports exactly one desktop
surface through the native Ginux bridge.

The controller may present that surface as a normal managed window or full
screen. It does not initially import individual Linux windows, implement a host
Wayland compositor, or grant the domain direct access to the physical seat.

### Failure and revocation

Connections, objects, buffers, focus, and leases are generation-bound. Client
disconnect, process failure, service restart, domain stop, timeout, or explicit
revocation destroys the affected generation, stops input delivery, releases or
scrubs buffers, and restores trusted controller focus. The CLI must remain
usable even when every graphical component has failed.

## Trust model

- The kernel trusts no protocol message, dimension, stride, damage region,
  object handle, or queue count until it is validated against the caller's
  capability and resource bounds.
- The compositor is trusted to render the physical display and route input, but
  cannot expand its boot-granted authority or modify the kernel.
- Native clients and Linux domains are mutually untrusted.
- Domain pixels are untrusted content. Trusted controller chrome and secure
  attention indicators use compositor-owned surfaces that clients cannot map,
  cover, or imitate as trusted state.
- Shared buffers become compositor-readable and client-nonwritable while
  submitted. Clients use another buffer for continued drawing.

## Consequences

- Ginux gains a graphical controller without making Linux the host desktop.
- The display protocol and toolkit become native ABI surfaces requiring explicit
  versioning, bounds, conformance tests, and compatibility policy.
- A separate graphical service contains most GUI failures and preserves the CLI,
  at the cost of IPC and lifecycle complexity.
- Linux application compatibility can grow inside the domain independently of
  the native host protocol.
- Software rendering is sufficient for the first experiment but does not prove
  physical GPU, DRM/KMS, broad Wayland, or desktop-environment compatibility.

## Rejected alternatives

### Put Wayland in the host controller

Rejected because it imports a large Linux-oriented protocol and implementation
surface into the host control plane and makes native controller policy depend on
a compatibility ecosystem Ginux does not otherwise require.

### Put all graphics in the kernel

Rejected because composition and window policy do not require ring 0, would
increase the privileged failure surface, and would make client failure recovery
harder.

### Give a focused domain the physical devices

Rejected for the first version because safe GPU/input transfer requires broader
driver reset, DMA, interrupt, firmware, and revocation contracts. Logical leases
provide the desired full-screen behavior while retaining controller recovery.

### Treat the Linux desktop as the controller

Rejected because the domain must remain a workload boundary. Linux root and its
display stack cannot receive implicit host-management authority.

## Validation

Gate 19 validates specification completeness. Gates 20–30 implement and test
the runtime, devices, protocol, focus/leases, window system, graphical
controller, domain bridge, Linux Wayland subset, presentation, recovery, and
qualification in that order.
