# Arm64 userspace daemon

## Source

Authorized by the human on 2026-11-06 for the fork
https://github.com/ericvh/ginux-arm64. Not an upstream product-gate rewrite.
Gates 9.75 and later stay the upstream sequence and are not part of this work.

## Goal

The Ginux userspace daemon builds and starts for aarch64 and still builds and
starts for x86_64. Each binary targets one architecture. The daemon's own
entry path and host syscalls follow that architecture.

## Approved scope

- Arch-specific startup and host syscall instruction sequences.
- Arch-specific host syscall numbers for the daemon's own kernel calls.
- aarch64 `clone(SIGCHLD)` where the kernel has no `fork`, and `ppoll` where
  it has no `pause`.
- `ginux arch` probe: print the architecture and complete one host fork/wait.
- `make build` remains the x86_64 daemon plus x86_64 fixtures.
- `make build-arm64`, `make arch-smoke`, and `make arch-smoke-arm64`.

## Non-goals

- Mediating an aarch64 domain's syscall ABI, registers, or ELF machine.
- Porting the x86_64 interpreter or the x86_64 assembly fixtures.
- Booting the legacy bare-metal kernel.
- Closing this gate without human review of its pull request.

## Completed work

- [x] Move daemon startup and host syscall ABI into `crates/ginux-init/src/arch/`.
- [x] Keep the x86_64 host numbers and instruction sequence byte-for-byte in role.
- [x] Add the aarch64 host path and the `ginux arch` probe.
- [x] Document the two build commands.

## Remaining work

- [ ] Human review of the pull request.

## Acceptance criteria

- `make build` produces the x86_64 `ginux` binary and the existing x86_64 fixtures.
- `make arch-smoke` prints `GINUX_ARCH: x86_64` and `GINUX_ARCH: fork=ok`, and `ginux help` exits 2 with usage text.
- `make build-arm64` produces an aarch64 ELF `ginux`.
- `make arch-smoke-arm64` prints `GINUX_ARCH: aarch64` and `GINUX_ARCH: fork=ok`, and `ginux help` exits 2 under qemu-user.
- x86_64 guest syscall numbers used by domain mediation are unchanged.

## Verification plan

- `make arch-smoke` -> x86_64 fork probe passes and usage exits 2.
- `make arch-smoke-arm64` -> aarch64 fork probe passes under `qemu-aarch64-static` and usage exits 2.
- `file` on both release binaries -> x86-64 and ARM aarch64, statically linked.
- `make build` -> x86_64 daemon and fixtures link.

## Evidence

- `make arch-smoke` -> `GINUX_ARCH: x86_64` and `GINUX_ARCH: fork=ok`.
- `make arch-smoke-arm64` -> `GINUX_ARCH: aarch64` and `GINUX_ARCH: fork=ok`.
- `file target/x86_64-unknown-linux-gnu/release/ginux` -> ELF 64-bit x86-64, statically linked.
- `file target/aarch64-unknown-linux-gnu/release/ginux` -> ELF 64-bit ARM aarch64, statically linked.
- x86_64 fixture packages `managed-payload`, `baseline-runner`, `runtime-fixture`, and `process-native-fixture` still link.

## Decisions and limitations

- Host and guest syscall numbers are separate. Guest numbers stay the x86_64 table until the mediation gate. `process_hooks::SYS_FORK` is that guest number, not the host fork.
- aarch64 `SYS_FORK` is the Linux `clone` number (220). `SYS_PAUSE` is the `ppoll` number (73). The x86_64 file keeps `fork` (57) and `pause` (34).
- x86_64 launch still issues host number 259 for its three-argument directory call. That historical number is `mknodat`. aarch64 uses `mkdirat` (34) because the call has mkdirat's shape. This does not change the x86_64 binary's number.
- `st_size` remains at offset 48 for the daemon's `fstat` buffer on both ABIs. The mediation register file is still the x86_64 `user_regs_struct` shape inside the aarch64 binary, so traced-domain supervision is not an arm64 ABI.
- The delivery environment requires branch prefix `cursor/` and suffix `-2b05`. That overrides the repository's usual `hdae/<slug>` branch name for this fork port.

## Delivery strategy

- Working branch: `cursor/arm64-userspace-daemon-2b05`
- Intended base: `main`
- Pull request: intended on https://github.com/ericvh/ginux-arm64
- Planned commits: one commit for the daemon ABI split, probe, and build targets.
- Actual commits: pending

## Review policy and outcome

The human authorized this stage and the two following stages as one batch, with a pull request when the stage builds. Technical passage does not close the gate. It stays in `gates-todo/` until the human accepts the pull request. The batch does authorize starting the next stage without waiting for that acceptance.
