# Native Controller Desktop Architecture

## Boundaries

1. **Kernel.** Owns physical framebuffer, keyboard, pointer, scheduling,
   isolation, memory, and capability enforcement. It does not implement window
   policy or graphical widgets.
2. **Controller services.** Trusted ring-3 Ginux processes own the physical seat
   and provide composition, window policy, resource leasing, and the existing
   versioned system-management interface.
3. **Controller clients.** The CLI and native graphical controller are separate
   clients of the same management interface. Neither receives hidden authority.
4. **Domains.** A Linux domain receives virtual display and input endpoints. A
   domain-local Wayland compositor combines its applications into one exported
   surface; the controller never trusts domain pixels or protocol messages.

## Governing invariants

1. The controller owns physical display and input devices by default.
2. Focus is explicit state, not an ambient consequence of receiving input.
3. Input is delivered only to the focused authorized surface.
4. Every resource lease names its owner, rights, lifetime, revocation path, and
   recovery behavior.
5. A secure-attention chord always returns input and display control to the
   controller, even while a domain is full screen.
6. Domain buffers are bounded, validated, isolated, and read only for the
   compositor after publication.
7. Clients cannot draw over trusted controller chrome or imitate the trusted
   attention and ownership indicators.
8. The CLI remains usable if graphical startup or a graphical client fails.
9. A compositor, controller-client, bridge, or domain failure cannot retain
   focus, device leases, or unreclaimed buffers.
10. Protocols are versioned and capability-scoped before compatibility grows.

## Display model

The native controller compositor manages a tree of surfaces. Native clients use
the Ginux display protocol directly. A Linux domain contributes one logical
desktop surface produced by its own compositor. That surface can be resized,
focused, minimized, or made full screen without changing domain identity.

Initial rendering is software based. Clients publish damage and completed
buffers; the compositor controls scanout timing and final composition. The
architecture leaves room for later acceleration without granting raw GPU access
or coupling the first protocol version to a particular device.

## Input and resource model

A seat groups keyboard, pointer, and display focus. Physical events enter the
kernel, are normalized by controller services, and are routed by the compositor
to the focused surface. Exclusive use is represented as a temporary logical
lease, not permanent device transfer. Raw-device passthrough is deferred.

## Linux integration

The domain bridge exposes a virtual display and virtual input device. Inside the
domain, a small Wayland compositor or bridge accepts ordinary Wayland clients,
composes them, and publishes the result through the native Ginux protocol. This
keeps Linux application compatibility inside the domain while Ginux retains the
host trust boundary and desktop policy.

## Failure model

Every connection and lease is generation-bound. Disconnect, crash, timeout,
domain stop, or controller restart invalidates its generation, returns focus to
the controller, revokes input delivery, and reclaims published buffers. The
serial CLI remains the recovery path throughout the project.
