# Arm64 userspace domain mediation

## Source

Authorized by the human on 2026-11-06 for the fork
https://github.com/ericvh/ginux-arm64, immediately after gate 15. Not an
upstream product-gate rewrite.

## Goal

An aarch64 build of the daemon supervises a tracee with the aarch64 Linux
register file and syscall numbers. The x86_64 build keeps the existing
ptrace register path and the same contract numbers.

## Approved scope

- `PTRACE_GETREGSET` / `PTRACE_SETREGSET` for aarch64 `user_pt_regs` and
  `NT_ARM_SYSTEM_CALL`.
- Translation between the generic aarch64 syscall table and the x86_64
  numbers the contract already matches, at the ptrace boundary only.
- Audit-architecture check for the binary's own ABI (`AUDIT_ARCH_AARCH64`
  or `AUDIT_ARCH_X86_64`).
- aarch64 `brk #0` where the fork parent plants a one-byte `int3`.
- Rewind of the 4-byte `svc` instruction when a wait is restarted.
- Unit tests for the name-matched number map, including the read/uname and
  openat/clone numeric collisions.

## Non-goals

- An aarch64 instruction interpreter. `ginux runtime` still decodes x86_64 ELF.
- Executing this mediation path on the current x86_64 host. qemu-user cannot
  ptrace an aarch64 tracee.
- Porting the legacy bare-metal kernel.
- Changing x86_64 contract numbers or the x86_64 `PTRACE_GETREGS` layout.

## Completed work

- [x] Arch-specific register read and write helpers.
- [x] Name-matched syscall map in `crates/ginux-core/src/syscall_map.rs`.
- [x] Target audit-arch rejection strings, with the x86_64 text unchanged.
- [x] Breakpoint word and syscall rewind helpers.

## Remaining work

- [ ] Human review of the pull request.
- [ ] Run the mediation path on an aarch64 Linux kernel. This environment
  cannot.

## Acceptance criteria

- `cargo test -p ginux-core` passes with the freestanding `RUSTFLAGS` cleared.
- `make build` still links the x86_64 daemon, and `make arch-smoke` passes.
- `make build-arm64` links, and `make arch-smoke-arm64` passes.
- aarch64 read 63 translates to x86_64 read 0, not uname 63.
- aarch64 openat 56 translates to x86_64 openat 257, and aarch64 clone 220
  translates to x86_64 clone 56.
- x86_64 `fork` (57) has no aarch64 number and therefore cannot be written
  back as a host syscall.

## Verification plan

- `RUSTFLAGS='' cargo test -p ginux-core --lib` -> all tests pass, including
  the two syscall-map tests.
- `make arch-smoke` and `make arch-smoke-arm64` -> fork probes pass.
- Both release `ginux` binaries link.
- Runtime ptrace of an aarch64 tracee is not available here; record that
  limit instead of substituting qemu-user.

## Evidence

- `RUSTFLAGS='' CARGO_TARGET_DIR=/tmp/ginux-unit cargo test -p ginux-core --lib`
  -> 11 passed.
- x86_64 and aarch64 release builds of `ginux-init` finished without warnings.
- `scripts/arch-smoke.sh` passed for `x86_64` and for `aarch64` under
  `qemu-aarch64-static`.

## Decisions and limitations

- The contract stays in x86_64 numbers so the large match tables do not fork.
  Only the aarch64 ptrace edge translates. Unknown numbers become a value
  that matches no contract arm, and writing them back uses syscall number -1
  so the host does not execute an unmapped call.
- Entry stops cache the six argument registers. Exit stops put the cached
  arguments back in `rdi`..`r9` and the real `x0` in `rax`. That matches the
  x86_64 property that argument registers survive a syscall.
- `cs` on the logical register file carries the stop phase only in the
  aarch64 build. `eflags` carries `pstate` so `SETREGSET` can write it back.
  The x86_64 build still passes the kernel `user_regs_struct` through.
- Fork-parent breakpoints use `brk #0` (`0xd4200000`) on aarch64 and `int3`
  on x86_64.
- `PTRACE_SYSEMU` is still the gate-1 resume request. A kernel without it
  fails that path closed, as on x86_64.
- This host is x86_64. The aarch64 mediation code was compiled and the daemon
  probe ran under qemu-user. No aarch64 ptrace stop was observed.

## Delivery strategy

- Working branch: `cursor/arm64-domain-mediation-2b05`
- Intended base: `cursor/arm64-userspace-daemon-2b05`
- Pull request: intended on https://github.com/ericvh/ginux-arm64
- Planned commits: mediation register file and syscall-number map.
- Actual commits: `0c31997c` Mediate aarch64 domain syscalls through the ptrace regset.

## Review policy and outcome

The human authorized gates 15 through 17 as one batch and asked for a pull
request when each stage builds. Technical passage does not close the gate.
The batch authorizes starting gate 17 without waiting for review of this pull
request.
