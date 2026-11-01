# Native Controller Desktop

## Purpose

This Ginux research iteration adds a native graphical controller environment
without turning the controller into a Linux desktop or adopting Wayland as the
host display server. Ginux will own the physical display and input seat, render
its own trusted desktop in ring 3, and present Linux domain desktops as
revocable surfaces within that environment.

The work is informed by GitHub issue #12, but it is a clean Ginux design rather
than a merge of the contributor prototype.

## Intended outcome

- Boot from the existing CLI controller into a native graphical controller.
- Provide a basic native windowing system and graphical controller application.
- Keep the CLI and graphical controller as peers over the same versioned
  management interface.
- Allow a Linux domain to run its own Wayland compositor and export one complete
  desktop surface to the controller.
- Switch a domain surface between a managed window and full-screen presentation.
- Keep physical devices under controller ownership, with explicit, temporary,
  and revocable leases for focused domains.
- Recover the controller desktop, input focus, and device ownership when a
  client or domain fails.

## Users

- Operators who configure Ginux, devices, networks, storage, and domains.
- Linux desktop users running an unmodified userspace inside a Ginux domain.
- Developers building native Ginux controller applications.

## Scope

The project covers a software-rendered native compositor, framebuffer and input
drivers, a versioned surface/input protocol, seats and focus, a basic windowing
toolkit, a native graphical controller, a domain display bridge, and a minimal
Wayland environment inside Linux domains. It includes deterministic QEMU tests,
failure recovery, performance budgets, and operator/developer documentation.

All Gates 19–30 use `codex/controller-desktop`, target `main`, and are intended
for one integrated pull request because the protocol, compositor, controller,
and domain bridge are one inseparable vertical feature.

## Non-goals

- GPU acceleration, DRM/KMS compatibility, or direct PCI GPU passthrough.
- Replacing the existing CLI controller.
- Running a host Wayland compositor in the controller layer.
- Full GNOME, KDE, clipboard, audio, accessibility, remote display, or
  multi-monitor support in the first iteration.
- Mixing individual Linux application windows into the native desktop; a Linux
  domain exports one composed desktop surface.
- Claiming the prototype is a production display or security architecture.

## Success

The batch succeeds when Ginux boots a responsive native controller desktop,
manages domains through a native graphical application, runs an unmodified
Wayland demonstration inside a Linux domain, presents that desktop windowed and
full screen, safely recovers focus and resources on failure, and passes its
deterministic QEMU and regression suites.
