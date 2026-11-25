# Unicorn Engine alternative execution backend

## Source

Gate 5 native-execution decision and Gate 3 interpreter evidence.

## Goal

Add a Unicorn Engine 2 x86-64 backend that executes managed instructions and
memory outside the host kernel while preserving the same Ginux OCI, VFS,
contract-dispatch, device, lifecycle, and audit interfaces.

## Deferred reason

The initial milestone prioritizes compatibility and progress through the Rocky
dynamic-loader and systemd gates. Native execution under ptrace supervision uses
the host kernel's mature ELF, CPU, and memory behavior while Ginux intercepts
every syscall before effect. Implementing and validating a second CPU backend
now would delay that path and is not required for Gate 5 acceptance.

The human reconsidered this option after accepting Gate 13 on 2026-11-10 and
reaffirmed that it belongs in the parking lot. Ginux prioritizes lightweight,
near-native execution; a CPU-emulation backend would add substantial
compatibility work and likely performance cost.

## Return condition

Reconsider only after explicit human direction, if a high-isolation emulated
backend becomes a product requirement, or if native supervision cannot satisfy
an approved boundary without unchecked syscall continuation. Do not reactivate
it merely because native performance or compatibility work is difficult.

## Proposed scope

- Integrate a pinned and reproducible Unicorn Engine 2 dependency.
- Map OCI-backed executable content and writable memory into an engine owned by
  Ginux rather than a native host tracee.
- Hook x86-64 `syscall` instructions into the existing Ginux dispatcher.
- Reuse instance configuration, virtual VFS, descriptors, contracts, devices,
  kernel views, lifecycle, and audit semantics across both backends.
- Define explicit backend selection and fail closed when the requested backend
  is unavailable.

## Non-goals

- Replacing the OCI image model or introducing an external OCI runtime.
- Weakening native-backend tests or treating Unicorn as automatic proof of
  production security.
- Multiple guest architectures in the first implementation.

## Proposed acceptance criteria

- The same pinned Rocky dynamic fixture runs under native and Unicorn backends
  with equivalent observable Ginux syscall decisions.
- A Unicorn-managed `syscall` cannot enter the host syscall implementation.
- Managed memory and register state remain engine/Ginux-owned and host pointers
  are not accepted as instance authority.
- Backend-differential tests identify semantic mismatches deterministically.
- Benchmarks quantify compatibility and overhead relative to native
  native ptrace supervision.

## Delivery strategy

- Dedicated `hdae/unicorn-execution-backend` branch based on the accepted
  predecessor state.
- Separate pull request; no push, merge, or execution is authorized while this
  gate remains deferred.

## Review policy and outcome

Human checkpoint. This gate changes the execution trust boundary and must be
explicitly reactivated before implementation.
