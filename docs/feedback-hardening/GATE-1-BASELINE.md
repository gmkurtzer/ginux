# Gate 1 Baseline and Claim Disposition

This record captures the starting point for the feedback-driven hardening
experiment. It does not treat a reproduced weakness as desired behavior.

## Baseline identity

- Published baseline: `research-v1-baseline`
- Working branch: `codex/feedback-hardening`
- x86-64 implementation size: 44,324 lines in
  `kernel/src/arch/x86_64/cpu.rs`
- Writable-global scan: 82 `static mut` declarations and 535 declarations
  with recognized interior-mutability types before classification.

## Claim disposition

| Review claim | Disposition | Reason |
|---|---|---|
| `SYSCALL` initially runs with the userspace RSP | Confirmed | The entry path saves registers through singleton globals before loading `USER_RETURN_STACK`; NMI and machine check can arrive in that interval. |
| NMI and machine check lack dedicated IST stacks | Confirmed | The IDT policy assigns a nonzero IST only to vector 8 (double fault). |
| User-copy has no fault recovery | Confirmed | Permission checks precede direct volatile access. A stable invalid pointer returns `EFAULT`, but a translation fault after validation has no fixup target. |
| Software bookkeeping overrides a rejected page-table walk | Rejected for the cited fast path | `current_linux_leaf_trusted` walks the active page tables and the caller checks the leaf's hardware presence and permissions. This does not remove the validation/access race above. |

Run `tools/architecture/reproduce-hardening-feedback.sh` to regenerate the
source-based disposition and `tools/architecture/inventory-writable-globals.sh`
to regenerate the machine-readable starting inventory.

## Baseline verification

| Command | Result | Time/observation |
|---|---|---|
| `make check` | Pass | Linux matrix: 82 complete, 245 partial, 48 policy-denied, 0 missing. |
| `make test-linux-abi` | Pass | 23 interfaces and 375 syscalls; 9.31 seconds. |
| `make test-storage` | Pass | Persistence and reset boots; 14.11 seconds. |
| `make test-domain-responsiveness` | Pass | Controller response measured at 7 ms; 16.32 seconds total. |
| `make test-rocky-systemd` | Pass | Three modes passed; Linux init to login was 6 seconds. |
| `make benchmark-compare` | Pass | Matching single-vCPU Rocky/Linux and Rocky/Ginux input set; existing medians reproduced. |
| `make test-multi-domain` | External failure | Registry transfer failed before the Alpine image became ready; QEMU was terminated by the harness trap. |
| `make test` / `make test-linux-runtime` | Baseline regression reproduced | The published tag fails at the linked ELF verifier. After repairing stale `AT_SYSINFO_EHDR` fixture expectations and loopback TCP endpoint classification, the branch reaches the later thread-transition probe and exposes an independent invalid return context. |

The ordinary boot failure is not hidden by the downstream harness message about
volume isolation: the kernel log records the actual earlier failure. The
remaining thread-transition failure is retained for subsequent entry,
ownership, and synchronization gates rather than weakening its assertions.

## Baseline repairs made before hardening

The branch fixes three stale or incorrect baseline checks without changing the
hardening claims:

1. reserve enough fixture stack space for the expanded auxiliary vector;
2. verify the `AT_SYSINFO_EHDR` entry in both Rust and assembly probes; and
3. close an accepted in-kernel TCP endpoint according to handle ownership after
   its peer has disappeared.

These repairs allow the boot suite to progress through a clean PID 1 lifecycle
and expose the later thread-transition failure that was previously masked.
