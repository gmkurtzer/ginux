# Contract and instance-network UX

## Source

Human review of the Gate 10.5 contract syntax and the distinction between a
portable instance recipe and host-specific network attachment.

## Goal

Make one portable instance recipe clearly describe the complete userspace
contract, while host-specific network attachment and port translation are
configured through repeatable `instance create` and `instance set` flags.

## Approved scope

- Replace the active `[userspace.*]` contract schema with `[contract]` plus
  `[contract.syscalls]`, `[contract.filesystem]`, `[contract.network]`,
  `[contract.devices]`, `[contract.privileges]`, `[contract.process]`, and
  `[contract.kernel]`.
- Give each policy surface an explicit default and deterministic deny
  precedence. Compile syscall-name globs against the pinned catalog; reject
  unmatched, unknown, or ambiguous rules.
- Represent setuid and setgid permission as executable-path arrays under
  `[contract.privileges]`; an empty array permits no set-ID execution.
- Keep `/proc` and `/sys` synthetic and Ginux-owned. Contract rules control
  visibility and permitted operations, never host-tree passthrough.
- Remove host interface selection and port forwarding from portable recipes.
- Add repeatable flags to both `ginux instance create NAME FILE` and
  `ginux instance set NAME`, including `--network ATTACHMENT` and
  `--forward protocol=...,external_port=...,instance_port=...`.
- Persist operational network settings separately from the portable recipe,
  support multiple changes in one command, and default an omitted attachment
  to the host's default network.
- Update validation, normalization, inspection, explanation, examples,
  documentation, and generated ABI coverage for the new model.
- Remove the replaced active syntax without compatibility aliases.

## Non-goals

- Adding kernel-resident enforcement, Landlock, cgroup, namespace, or
  capability optimizations; those belong in a separately approved Gate 12.5.
- Changing Gate 11 performance budgets, dispatch semantics, or evidence.
- Adding new network backends, orchestration, or live migration.
- Editing locked historical gate records.

## Completed work

- [x] Human aligned and approved the schema, network ownership boundary,
  repeatable create/set flags, glob behavior, and Gate 12 checkpoint.
- [x] Created dedicated stacked branch `hdae/contract-network-ux` from
  `hdae/performance-characterization` while pull request 249 remains open.
- [x] Implemented and normalized the hierarchical contract schema, including
  syscall globs, deterministic deny precedence, recursive policy paths,
  set-ID path lists, process limits, and synthetic kernel-view policy.
- [x] Separated host network attachment and translated ports into persistent
  deployment state with repeatable, atomic create/set flags.
- [x] Migrated active recipes, generated ABI coverage, benchmark tooling,
  operator documentation, and contract/runtime assertions.
- [x] Added Rocky integration coverage for multiple create/set flags, host
  default attachment, portable recipe preservation, and atomic failure.
- [x] Ran the complete configuration, instance, contract, runtime, network,
  systemd, aggregate, and human demonstration evidence.

## Remaining work

- [ ] Obtain human review and acceptance.
- [x] Push the branch and open the Gate 12 pull request after explicit batch
  authorization.

## Acceptance criteria

- A self-contained recipe expresses all approved contract surfaces with clear
  defaults; old `[userspace.*]` contract keys fail closed.
- Syscall globs expand deterministically against the pinned catalog, must match
  at least one syscall, and cannot silently expand when a catalog version is
  unchanged. Explicit deny wins over allow.
- Filesystem and device policies use consistent read/write/create semantics;
  setuid and setgid use executable-path arrays; process limits include both
  processes and threads.
- `/proc` and `/sys` remain synthetic and expose only effective-contract state.
- Portable recipes contain no host interface, source address, or port-forward
  ownership.
- Create and set accept multiple repeatable network flags atomically; invalid
  combinations change no persisted state.
- Omitted `--network` uses the host default. Forwarding syntax names both
  `external_port` and `instance_port`, and conflict detection remains enforced.
- Existing systemd, lifecycle, contract-denial, and Gate 11 dispatch behavior
  remain passing.

## Verification plan

- `make check` -> formatting, build, and static policy checks pass.
- `make test-config` -> new schema and operational override matrix pass;
  replaced syntax and invalid globs fail closed.
- `make test-instance` -> create/set persistence, atomic multi-flag updates,
  default attachment, and restart behavior pass.
- `make test-contract` and `make test-contract-runtime` -> compiled decisions,
  deny precedence, glob expansion, set-ID, device, process, and virtual-kernel
  policies pass.
- `make test-network` -> attachment, forwarding, conflict, and cleanup behavior
  pass using `external_port` and `instance_port`.
- `make test-systemd` and `make test` -> mandatory regressions exit zero.
- `make demo-gate-12` -> a human can inspect one recipe, create an instance with
  multiple flags, update multiple settings atomically, inspect effective state,
  and observe one allowed and one denied contract action.

## Evidence

- `make check` -> passed formatting, static policy checks, generated-document
  freshness, and cross-target release lint.
- `make test-contract` -> 13 contract and 23 instance tests passed, including
  syscall-glob expansion, unmatched-glob rejection, and deny precedence.
- `ROCKY_SSH_PORT=22413 make test-config` -> passed the migrated schema,
  normalization, explanation, invalid selector, and legacy-command matrix.
- `ROCKY_SSH_PORT=22414 make test-instance` -> passed existing lifecycle
  coverage plus repeatable create/set flags, two simultaneous forwards,
  default attachment, portable recipe separation, and atomic failed updates.
- `ROCKY_SSH_PORT=22415 make test-contract-runtime` -> paranoid and selective
  denial both blocked the socket operation before its host effect while
  preserving virtual identity.
- `make test-network` -> passed bounded, isolated, revocable virtual-network
  behavior.
- `ROCKY_SSH_PORT=22419 make test-systemd` -> cold, warm, repeat, offline DNF
  failure, configured startup failure, and controlled shutdown all passed with
  the explicit network contract.
- `ROCKY_SSH_PORT=22417 make test` -> complete aggregate unit and Rocky QEMU
  regression suite passed.
- `ROCKY_SSH_PORT=22418 make demo-gate-12` -> displayed the complete
  hierarchical Rocky recipe, repeatable deployment commands, one compiled
  allowed operation, one runtime-denied operation, and ended with
  `GATE12: PASS`.

## Decisions and limitations

- The portable recipe owns guest-visible policy; host-specific attachment owns
  deployment state and is persisted separately.
- Character `*` globs are bounded to syscall names and compile against a pinned
  catalog. Resource selectors retain their documented component-aware grammar.
- Gate 12 changes configuration and user experience, not the enforcement
  architecture or Gate 11 performance result.
- The runtime still materializes an internal effective `runtime.toml` from the
  portable recipe and separate deployment state. Host interface/source and
  translated-port fields may appear only in that derived runtime input, not in
  the saved portable `config.toml`.
- Because pull request 249 is still open, this branch is stacked on Gate 11 and
  must be rebased or retargeted after that dependency is resolved.

## Delivery strategy

- Working branch: `hdae/contract-network-ux`
- Intended base: `hdae/performance-characterization` while pull request 249 is
  open; retarget to `main` after Gate 11 lands
- Pull request: https://github.com/gmkurtzer/ginux/pull/251; never merge or
  close without explicit authorization
- Planned commits: gate and architecture record; contract schema/compiler;
  operational network CLI/state; active recipe and documentation migration;
  regression and demonstration evidence
- Actual commits: `f905e9e8` gate and architecture record; `0032227d`
  operational deployment state and CLI; `f9683745` hierarchical contract and
  recipe migration; `438e38c6` atomic deployment integration coverage;
  `040f8474` Gate 12 regression and demonstration target; `1d8cbc62` final
  evidence record; `35ee4d9b`, `a2e33716`, and `e63d3f98` bounded lifecycle,
  audit, and attached-console corrections

## Review policy and outcome

Technically passed and delivered for human review in pull request 251. Human
acceptance remains pending. The human authorized the subsequent corrective and
UX gate batch on 2026-11-08; later stacked gates do not imply Gate 12 acceptance
or authorize this pull request to be merged or closed.
