# Gate 15: Linux Personality Decomposition

## Source

GitHub issue #5, `cpu.rs` decomposition finding.

## Goal

Reduce architecture-file ownership of Linux personality code without changing
observable behavior.

## Approved scope

- Extract Linux random, boot-ID, and getrandom policy into a focused module.
- Preserve runtime ownership, ABI behavior, and emitted kernel verification.

## Non-goals

- Do not redesign syscall dispatch or complete the full decomposition at once.
- Do not close issue #5 or merge the pull request.

## Completed work

- [x] Extracted random, boot-ID, and getrandom policy into `linux_random.rs`.
- [x] Preserved the runtime-owner backing declarations and inventory.
- [x] Reduced `cpu.rs` from 42,876 to 42,752 lines.

## Remaining work

- [x] Run static and QEMU verification.
- [x] Push the existing branch and update pull request #9.

## Acceptance criteria

- Linux random and boot-ID behavior no longer lives in the architecture file.
- Runtime-owner and writable-global checks pass.
- Kernel clippy and the complete QEMU verification boot pass.

## Verification plan

- `make architecture-check` -> passes the ownership inventories.
- `make check` -> passes formatting, lint, unit, and architecture checks.
- `make test` -> complete QEMU verification boot passes.

## Evidence

- `make check` -> passed.
- `make test` -> passed with `GINUX_KERNEL_OK` and `Boot test passed.`
- `wc -l` -> `cpu.rs` 42,752 lines; extracted module 121 lines.

## Decisions and limitations

- This is one mechanical extraction, not a claim that the full personality
  decomposition is complete.
- The user explicitly directed the corrective work into existing pull request
  #9, overriding the default one-gate-per-PR delivery split.

## Delivery strategy

- Working branch: `codex/issue-5-criticism-reconciliation`
- Intended base: `main`
- Pull request: existing PR #9
- Planned commits: decomposition and verification; delivery checkpoint
- Actual commits: `0a3078ec` (decomposition and verification integration)

## Review policy and outcome

Delegated by the saved proactive scheduled-review policy; remains open for
maintainer review in pull request #9.
