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

## Return condition

Reconsider this gate after the native ptrace path runs the Gate 9
systemd baseline and Gate 11 has measured its performance, and before Ginux
makes production-boundary claims. Reconsider it earlier if native supervision
cannot satisfy a gate without unchecked syscall continuation, or if the exposed
host ELF, memory, signal, fault, scheduling, or ptrace surface is unacceptable.

## Proposed scope

- Integrate a pinned and reproducible Unicorn Engine 2 dependency.
- Map OCI-backed executable content and writable memory into an engine owned by
  Ginux rather than a native host tracee.
- Hook x86-64 `syscall` instructions into the existing Ginux dispatcher.
- Reuse domain configuration, virtual VFS, descriptors, contracts, devices,
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
  are not accepted as domain authority.
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
