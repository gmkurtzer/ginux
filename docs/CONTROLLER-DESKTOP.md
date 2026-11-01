# Native Controller Desktop Specification

**Status:** Gate 19 architecture checkpoint  
**Protocol:** Ginux Native Display v0 (`GND0`)  
**Scope:** software-rendered QEMU display and one controller-owned seat

## 1. Product behavior

Ginux boots its existing serial CLI first. The controller may then start a
native graphical service and graphical controller application. The CLI remains
usable at all times. A graphical failure is reported through the controller and
does not replace or restart it.

The native desktop provides trusted system chrome, managed native windows, and
one window for each presented domain desktop. A domain window may enter full
screen through a controller lease. Secure attention always returns to the
trusted native desktop.

## 2. Component and authority map

| Component | Context | Owns | Must not own |
|---|---|---|---|
| Kernel display/input drivers | Ring 0 | physical device state, DMA/MMIO/IRQ where applicable, bounded queues, grants | window policy, widgets, domain UI |
| Resident controller | Ring 3, initial control domain | management state, service supervision, delegation/revocation | pixel composition, hidden GUI-only state |
| Native compositor | Ring 3, controller-supervised service | scanout, trusted chrome, surfaces, focus, logical leases | authority beyond delegated display/input/management subset |
| Graphical controller | Ring 3 native client | its surfaces and UI state | canonical system state, physical devices |
| Native application | Ring 3 client domain | its protocol objects and writable drawing buffers | other clients' objects, trusted chrome, physical input |
| Domain bridge | Kernel plus bounded ring-3 endpoint | one virtual display/input channel per domain | physical seat ownership, other-domain data |
| Linux compositor | Linux personality domain | Wayland socket, Linux client windows, one composed desktop surface | host management or physical device authority |

All rights originate in the boot authority envelope and are attenuated through
generation-safe capabilities. Process ancestry supplies lifecycle ownership,
not authority.

## 3. Runtime topology

```text
physical display + keyboard + pointer
                 |
       kernel device interfaces
                 |
       native controller compositor
          /             |             \
 trusted chrome   graphical control   domain desktop surface
                           app                  |
                                         domain bridge
                                                |
                                     Linux Wayland compositor
                                      /                    \
                              Wayland client A      Wayland client B
```

The compositor is supervised by the resident controller and uses a distinct
address space and capability table. The graphical controller is a client of the
compositor and the existing management interface. Linux domains never connect
to the physical input or display objects.

## 4. GND0 transport

### 4.1 Connection

A client opens a capability-authorized IPC endpoint. The server creates a
connection generation and returns its supported protocol major/minor and feature
bits. The client selects a compatible version before creating objects.

Connection states are:

```text
NEW -> NEGOTIATED -> ACTIVE -> CLOSING -> DEAD
 |         |           |          |
 +---------+-----------+----------+--> DEAD on validation or peer failure
```

Only `NEGOTIATE` is legal in `NEW`. Object operations require `ACTIVE`.
Incompatible major versions, unknown required features, malformed messages, or
capability failure close the connection. Optional unknown messages return a
versioned unsupported-operation error without changing state.

### 4.2 Message envelope

Every message uses a fixed-width little-endian envelope:

| Field | Width | Meaning |
|---|---:|---|
| `magic` | 32 bits | `GND0` |
| `major` / `minor` | 16 / 16 bits | negotiated protocol version |
| `opcode` | 16 bits | request or event identifier |
| `flags` | 16 bits | required/optional and request/event flags |
| `length` | 32 bits | complete aligned message length |
| `object` | 64 bits | opaque generation-safe object handle |
| `sequence` | 64 bits | monotonic per-connection sequence |

The implementation defines conservative compile-time maxima for message length,
object count, dimensions, stride, buffer bytes, damage rectangles, queued
events, and outstanding frame callbacks. Limits are queryable but cannot exceed
the caller's capability/resource budget. Arithmetic is checked before mapping,
allocating, copying, or multiplying dimensions and strides.

Sequences are exact and monotonic. Replay, gaps where exact order is required,
cross-connection handles, stale generations, and wrong object types fail closed.

### 4.3 Object model

| Object | Created by | Purpose |
|---|---|---|
| `registry` | server | advertises authorized globals and versions |
| `compositor` | registry bind | creates client surfaces |
| `surface` | compositor | receives attach, damage, commit, and presentation state |
| `pool` | client grant | bounds one shared-memory grant |
| `buffer` | pool | typed pixel view with offset, dimensions, stride, and format |
| `output` | registry bind | reports logical size, scale, refresh, and configure events |
| `seat` | registry bind | reports capabilities and creates keyboard/pointer objects |
| `keyboard` | seat | keymap, focus, key, modifiers, and repeat metadata |
| `pointer` | seat | focus, motion, button, axis, and capture state |
| `callback` | surface commit | one-shot frame-completion notification |
| `lease` | privileged controller operation | temporary exclusive presentation/input rights |

Clients may destroy their objects in dependency order. Disconnect destroys all
remaining objects server-side. The server owns global and trusted-chrome objects;
client handles can never name them.

## 5. Buffer and surface lifecycle

### 5.1 Buffers

Buffers use explicit ownership states:

```text
CLIENT_WRITABLE -> SUBMITTED -> COMPOSITOR_READABLE -> RELEASED
       ^                                                  |
       +--------------------------------------------------+
```

Creation validates format, offset, dimensions, stride, checked byte extent,
grant rights, and resource budget. `SUBMITTED` attenuates the client's mapping
so it cannot write while the compositor reads. A client renders concurrently by
using another writable buffer. Release restores client write rights only if the
connection and generation remain live; otherwise the grant is destroyed.

Accepted v0 formats are an explicitly enumerated 32-bit packed RGB format and a
32-bit packed format with alpha. No implicit color-space conversion exists.

### 5.2 Surfaces

Surface states are:

```text
UNCONFIGURED -> CONFIGURED -> MAPPED <-> HIDDEN -> DESTROYED
                     |          |
                     +----------+--> DESTROYED
```

State changes are atomic at `COMMIT`. Pending buffer, damage, position metadata,
and frame callback either become current together or remain pending after a
rejected commit. Mapping requires a valid configure acknowledgement and buffer.

Damage rectangles are clipped to the buffer and surface bounds. An empty damage
set permits server reuse; it does not imply that unchecked coordinates are safe.
The compositor sends a frame callback after the committed content is no longer
needed for that frame, not merely after request receipt.

## 6. Window policy and trusted UI

The native window manager is part of the compositor service. It owns desktop
background, panel, notifications, decorations, cursor, focus indicators, and
secure-attention UI. Clients own only content surfaces.

Required v0 operations are create/destroy, move, resize, minimize, maximize,
restore, close request, z-order activation, and full-screen request. A client
request is policy input, not authority to cover trusted chrome or take focus.

Trusted UI is drawn from compositor-owned buffers after client composition.
The compositor reserves a visually stable trust indicator that includes current
focus and exclusive-lease owner. Client content is always clipped below trusted
overlays.

## 7. Seat, focus, and input

One v0 seat contains one keyboard and one relative pointer. The seat tracks:

- keyboard focus;
- pointer focus;
- optional pointer capture;
- active presentation/input lease; and
- secure-attention generation.

Focus states are:

```text
TRUSTED_CONTROLLER -> NATIVE_CLIENT -> DOMAIN_SURFACE
        ^                 |                 |
        +-----------------+-----------------+
                 secure attention / owner failure
```

The compositor changes focus only after validating surface liveness, visibility,
seat capability, and policy. Focus loss generates a release/cancel boundary so
keys and buttons cannot remain logically pressed for the next owner. Unfocused
clients receive no physical input.

The secure-attention chord is a platform-owned configurable chord processed
before ordinary routing. It cannot be intercepted or disabled by clients. Its
exact default binding is selected during implementation without changing the
state contract.

## 8. Resource leases

A lease is an opaque generation-safe record containing:

- resource and seat identity;
- owner domain/process/surface generation;
- rights (`PRESENT_EXCLUSIVE`, `KEYBOARD`, `POINTER`, or a subset);
- grant and expiration time;
- revoker authority;
- current state and reason for the last transition.

Lease states are:

```text
REQUESTED -> GRANTED -> ACTIVE -> REVOKING -> REVOKED
     |          |          |          |
     +----------+----------+----------+--> DENIED/EXPIRED
```

Only one active exclusive-presentation lease exists per output. Activation is
transactional with focus change. Revocation first blocks new input, synthesizes
release/cancel events, restores trusted composition, then destroys the lease.
Owner death, timeout, domain stop, generation mismatch, secure attention, or an
authorized controller request initiates revocation.

Raw MMIO, DMA, interrupt, GPU, keyboard, and pointer-device capabilities are not
part of GND0 leases.

## 9. Domain bridge

Each authorized domain bridge binds one domain generation to:

- one virtual output and one exported native surface;
- a bounded set of shared display buffers;
- one damage/frame-completion queue; and
- one virtual keyboard/pointer event queue.

The bridge labels all handles with the domain generation and validates every
operation against it. Stop/restart invalidates the complete endpoint. Queue
backpressure skips or coalesces safe redraw work rather than blocking the
controller; input events are never silently rerouted to another generation.

Inside a Linux personality domain, a small compositor exposes a normal Wayland
socket. Selected unmodified clients use shared-memory buffers and ordinary
input/output interfaces. The Linux compositor combines its client windows into
one domain framebuffer and speaks GND0 only through the bridge endpoint.

The initial compatibility set is recorded by Gate 27 from observed client
protocol use. Unsupported required globals are absent or return explicit
protocol failure; they must not hang indefinitely.

### Initial Linux Wayland compatibility set

The bounded domain compositor publishes `/run/user/0/wayland-0` and models the
ordinary client-facing objects needed by the initial demonstration workload:
`wl_display`, `wl_registry`, `wl_compositor`, `wl_shm`, `wl_seat`, and
`xdg_wm_base`. The selected clients create one shared-memory surface each,
acknowledge `xdg_surface` configuration, attach and damage a buffer, request a
frame callback, and consume pointer or keyboard events. Two clients may be live
and composed concurrently into the single exported domain desktop.

The implementation is intentionally bounded to four clients, eight windows,
and 32 queued events per client. Unknown globals and requests fail explicitly.
Client disconnect destroys its windows, invalidates its generation, and leaves
other clients live; domain reset invalidates every client and exported frame.
This first subset does not advertise X11, Xwayland, GPU, clipboard, audio,
accessibility, decoration, data-device, or broad desktop-shell compatibility.

## 10. Lifecycle and recovery

### Managed domain presentation

The controller represents one running Linux domain desktop with one stable
presentation object. Its last completed frame remains visible while a bounded
resize configure is outstanding; accepting the matching configure serial
replaces only the generation-bound bridge endpoint. Entering full screen keeps
the same domain surface and frame sequence while granting a logical exclusive
lease. It does not restart the domain or transfer a physical device.

Controller-rendered chrome identifies the domain and whether it owns focus or
an exclusive lease. Secure attention is handled before domain input, returns
the view to its prior managed rectangle, revokes exclusive ownership, and
prevents further physical input until focus is explicitly granted again.
Minimize, stop, failure, and restart likewise clear focus and leases. Stop and
failure clear domain pixels and show distinct controller-owned state; restart
creates a fresh bridge generation with no inherited input or frame queue.

| Failure | Required response |
|---|---|
| Native client exits | destroy its objects, revoke capture/lease, damage exposed desktop |
| Graphical controller exits | preserve system state and CLI; allow clean restart |
| Compositor fails | kernel/controller revoke device grants, restore serial recovery, reclaim buffers, restart only under bounded policy |
| Domain bridge fails | revoke domain presentation/input endpoint; preserve domain lifecycle control |
| Linux compositor fails | show domain display failure; retain controller focus; allow domain or service restart |
| Domain stops/restarts | invalidate generation, revoke lease/focus, destroy buffers/events |
| Event/buffer limit exceeded | reject or apply documented backpressure to owner only |
| Secure attention | preempt routing, restore trusted controller presentation, revoke exclusive lease |

Restart never reuses an old generation. Resource accounting covers processes,
address spaces, frames, mappings, capabilities, protocol objects, event queues,
callbacks, and leases.

The recovery supervisor records a bounded diagnostic stream naming the failed
component, retired generation, recovery result, and monotonic sequence. Faults
immediately restore trusted focus and clear presentation leases, surfaces,
buffers, event queues, and handles. Restarts use a fresh generation after a
bounded retry delay; three consecutive failed attempts exhaust the automatic
restart budget and leave the serial CLI as the recovery path. Successful
recovery resets the consecutive-failure budget.

## 11. Management integration

The graphical controller and CLI query and mutate the same canonical management
objects. The GUI requires no private kernel calls. It must expose:

- system, CPU, memory, device, network, and storage status;
- domain creation, configuration, lifecycle, attach, and exec;
- active focus, seats, resources, and leases;
- asynchronous operation progress and failures;
- graphical-service and domain-display health; and
- clean reboot and shutdown through a confirmation-safe power menu.

A slow domain operation runs asynchronously and cannot block input, composition,
or the CLI.

## 12. Visual system

The controller desktop uses a restrained dark navy and slate palette with blue
selection and mint health accents. A persistent top bar identifies Ginux,
system management, and overall health. The left navigation provides Overview,
Domains, Network, Storage, Devices, Events, and Settings destinations. Settings
is the stable home for system-wide configuration, beginning with date, time,
and timezone controls and expanding as additional host configuration is added.
The Overview page summarizes domain, CPU, and memory state and shows a compact
live domain table. The serial controller remains an independent recovery and
automation interface rather than being duplicated as a graphical prompt.

The boot path selects an exact 1024 by 768 GOP mode when firmware exposes it,
and the native renderer fills that surface without a nested virtual window,
fake window controls, graphical prompt, or unused margins. The controller maps
only the capability-authorized framebuffer pages into its address space and
writes them directly, avoiding the former per-pixel syscall copy path. Pointer
motion restores and repaints only its bounded 18 by 20 saved-under region.
Controller text uses an embedded, antialiased Inter atlas generated at build
time; icons and the high-contrast pointer remain allocation-free software
primitives. Palette and major layout dimensions are centralized in explicit
`Theme` and `Layout` contracts so the shell can be restyled without changing
management behavior or hit testing.
Color is never the only status signal: labels, values, and state text remain
present. The accepted visual reference is
`docs/images/controller-desktop-concept-v2.png`; actual QEMU framebuffer
evidence is retained under
`build/qualification/controller-desktop/` after visible qualification.

## 13. Verification map

| Architectural property | Implementing gate | Required evidence |
|---|---:|---|
| Ring-3 graphical service and CLI recovery | 20 | launch/fault/restart QEMU test |
| Physical output/input under controller ownership | 21 | pixel and injected-input fixtures |
| Versioned bounded GND0 objects and buffers | 22 | conformance and malformed-message tests |
| Focus, leases, secure attention, revocation | 23 | transition, race, stale-generation, SMP tests |
| Native windows, text, widgets, trusted UI | 24 | scripted input and screenshot fixtures |
| Graphical system management | 25 | CLI/GUI state-equivalence workflows |
| Per-domain virtual display/input isolation | 26 | multi-domain negative and lifecycle tests |
| Unmodified selected Wayland clients | 27 | domain-local client protocol tests |
| Managed/full-screen domain desktop | 28 | end-to-end focus/resize/secure-attention test |
| Fault recovery and resource reclamation | 29 | failure matrix and repeated SMP stress |
| Performance, regression, and documentation | 30 | clean full qualification and measured budgets |

## 14. Qualification and measured budgets

Run the complete desktop qualification from the repository root:

```sh
make test-controller-desktop-all
make test-controller-desktop-performance
make test-controller-desktop-visible
make test-controller-desktop-latency
make run-gui
```

The first command builds a verification image, runs every native desktop
contract, boots that image in QEMU, and repeats the recovery boot with two and
four virtual CPUs. It covers controller startup, native windows and management,
two-domain isolation, the bounded Wayland subset, managed and full-screen
presentation, secure attention, generation retirement, and fault recovery.

`make test-controller-desktop-visible` boots the ordinary runtime, captures the
actual emulated display through QEMU, injects PS/2 keyboard and relative-pointer
input, and verifies navigation, live domain data, and the power menu in complete
1024 by 768 controller desktop frames. The same run measures the complete
QEMU event-to-visible-frame path; `make test-controller-desktop-latency` is the
explicit latency gate. It retains baseline, keyboard, pointer, live-state,
power-menu, and latency evidence in
`build/qualification/controller-desktop/`. On macOS, `make run-gui` opens
the same runtime in a Cocoa window; click that window to focus its physical
keyboard and pointer while the serial controller remains in the invoking
terminal. Set `GINUX_QEMU_DISPLAY` to another QEMU display backend when needed.

The performance command runs the release-mode host model at 1024 by 768 pixels
for 500 samples. The input-to-frame path keeps a second domain active, consumes
one focused input event, publishes the resulting foreground frame, and renders
it. The steady-frame measurement renders an already-published frame. Recovery
cycles one supervised component through fault containment and fresh-generation
restart. The memory figure is the modeled bounded allocation for two
presentations, the supervisor, the pixel buffer, and measurement samples; it is
not process RSS.

Measurements recorded on 2026-11-01 on an Apple Silicon macOS host with QEMU
10.1.2 were:

| Metric | Measured | Budget | Result |
|---|---:|---:|---|
| Two-presentation model startup | 1 us | 50,000 us | pass |
| Input-to-frame p95 with background domain | 1,317 us | 16,667 us | pass |
| Steady software frame p95 | 959 us | 16,667 us | pass |
| Fault/recovery cycle p95 | <1 us | 1,000 us | pass |
| Modeled bounded working set | 8,324,400 bytes | 8,388,608 bytes | pass |

These budgets are regression limits for the implemented state machines and
software renderer. They are not GPU, physical-device, whole-system CPU,
resident-memory, or interactive desktop benchmarks. Raw output is retained at
`build/qualification/controller-desktop-performance.txt` after each run.

## 15. Explicit initial limitations

GND0 does not initially provide GPU acceleration, DRM/KMS, direct device
passthrough, multiple outputs, touch, clipboard, drag-and-drop, audio,
accessibility, remote display, X11, Xwayland, complete Wayland coverage, or a
general third-party toolkit ecosystem. These require later decisions and gates;
none may be inferred from passing this batch.
