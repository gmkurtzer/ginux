# Declarative operator CLI and internal command boundary

## Source

Human review of pull request 272 and the confusing internal command form
`ginux native <config> <root> ...`. The intended operator workflow is the named
instance lifecycle (`ginux create`, `import`, `start`, and related commands),
while `native` is currently an internal runtime/backend selector invoked by the
privileged instance helper.

## Goal

Provide one coherent, discoverable Ginux operator CLI backed by a mature
declarative command framework, and establish an explicit boundary that keeps
runtime, fixture, helper, and backend commands out of the public interface.

## Approved scope

- Inventory every command and option accepted by the public controller,
  privileged instance helper, runtime binary, tests, and maintenance scripts;
  identify its callers and classify it as public, internal, test-only, or
  obsolete.
- Derive the public command taxonomy from complete operator workflows,
  including instance creation/import, configuration, inspection, lifecycle,
  attachment, maintenance locking, and recipe diagnostics.
- Present the proposed command names, grouping, options, defaults, exit
  behavior, compatibility impact, and removals for human approval before
  implementation. In particular, decide whether internal selectors such as
  `native`, `native-hook`, `process-native`, `launch`, and fixture commands are
  removed, renamed, or retained behind an explicitly private runtime protocol.
- Replace the hand-written shell parsing in the public `/usr/local/bin/ginux`
  controller with a compiled declarative CLI using Rust `clap`, or an
  equivalently maintained command framework with a documented reason for the
  substitution.
- Define commands, positional arguments, options, conflicts, repeatability,
  defaults, value validation, help text, and version output from one
  authoritative command model.
- Preserve the current privilege boundary: ordinary parsing and presentation
  remain unprivileged; only narrowly defined state-changing operations cross
  into the privileged helper; the workload runtime remains a separate internal
  executable.
- Replace ad hoc public parsing without requiring the runtime's freestanding
  `no_std` execution path to link the operator CLI framework. If an executable
  boundary is still required internally, give it an explicit private contract
  and deterministic validation rather than presenting it as operator syntax.
- Add deterministic parser/help tests and end-to-end workflow coverage for
  valid commands, invalid commands, missing and extra arguments, unknown and
  conflicting options, repeatable options, `--help`, and `--version`.
- Update installation, documentation, examples, demonstrations, and active
  tests to show only the approved operator interface and clearly identify any
  documented maintainer-only diagnostic surface.

## Non-goals

- Changing syscall mediation, virtual-kernel behavior, instance contracts,
  image acquisition, or workload execution semantics.
- Changing the instance recipe schema or migrating existing instance data.
- Exposing internal runtime, helper, backend, or fixture commands merely
  because they currently have an argument parser.
- Preserving compatibility aliases for rejected command shapes unless the
  human explicitly approves a bounded migration period at the design
  checkpoint.
- Rewriting privileged instance-management behavior that can remain behind a
  narrow, tested interface.

## Completed work

- [x] Confirmed that `/usr/local/bin/ginux` is currently a shell controller and
  that `native` is not accepted by its public command dispatcher.
- [x] Confirmed that the instance helper invokes the separate internal runtime
  as `ginux-runtime native <runtime-config> <root> configured` during
  `ginux start`.
- [x] Recorded the human direction to use a Cobra-like declarative framework
  and re-evaluate the entire CLI before implementation.

## Remaining work

- [ ] Produce the command/caller inventory and proposed public/internal
  boundary.
- [ ] Present the proposed public command specification and compatibility plan,
  then stop for human design approval.
- [ ] Implement the approved declarative public CLI and narrow internal
  protocol on this dedicated branch.
- [ ] Add parser, help, failure, installation, and end-to-end workflow tests.
- [ ] Update active operator and maintainer documentation and examples.
- [ ] Run all criterion-mapped verification and record fresh evidence.
- [ ] Publish the dedicated pull request and stop for final human review.

## Acceptance criteria

- A checked-in command inventory accounts for every accepted public,
  privileged-helper, runtime, and test-only command and names all known callers;
  no command is silently retained or removed.
- The human approves the final public taxonomy and compatibility/removal plan
  before implementation begins.
- The installed `ginux` executable uses a mature declarative parser rather than
  hand-written shell argument dispatch, and provides consistent global and
  per-command help plus version output.
- The ordinary instance lifecycle remains clear and functional through the
  approved forms of create/import, list/status, start/attach, stop,
  configuration, inspection, export/edit, and lock/unlock workflows.
- Public help, errors, examples, and completion-visible command metadata do not
  expose private runtime/backend selectors such as `native` unless the human
  explicitly promotes one at the design checkpoint.
- Every accepted command rejects missing, extra, unknown, malformed, or
  conflicting input with a concise diagnostic, stable nonzero exit status, and
  relevant usage without performing the requested state change.
- Repeatable deployment options such as network attachments, forwarding, and
  typed overrides retain their documented ordering and atomic failure
  behavior.
- The public CLI does not gain privilege merely to parse, format help, inspect
  local input, or attach to an already-authorized console; privileged
  operations remain narrow and explicit.
- Existing recipes, instance directories, image caches, and stopped/running
  lifecycle state remain compatible unless a separately approved migration is
  recorded before implementation.
- Installation produces the intended public `ginux` binary and separate
  internal executables with no ambiguous public alias to the runtime.
- Active README examples and CLI documentation exactly match generated help and
  the tested operator workflow.
- Existing configuration, instance, systemd, interactive-console, contract,
  security, and aggregate regressions remain green.

## Verification plan

- Command-inventory check -> compares public help and internal dispatch tables
  with the checked-in classification and fails for an unclassified command or
  caller.
- CLI parser unit/snapshot suite -> verifies global and per-command help,
  version output, positional arity, option values, conflicts, repeatability,
  `--` handling where applicable, diagnostics, and stable exit statuses.
- Negative state-safety suite -> malformed and rejected invocations leave
  recipes, deployment state, roots, caches, locks, and running instances
  unchanged.
- `make test-cli` -> builds the installed command shape and runs deterministic
  public CLI, help, parser, privilege-routing, and installation assertions.
- `make test-instance test-config` -> complete recipe, override, inspection,
  and lifecycle component workflows pass through the approved CLI.
- `make test-systemd-interactive` -> a clean and cached Rocky host exercises
  create/import, start/attach/detach, status, audit, stop, restart, and
  persistence using only the public CLI.
- `make test-systemd test-contract test-security test` -> runtime,
  policy-boundary, and aggregate regressions remain green.
- `make demo-gate-25` -> displays global and command help, demonstrates the
  approved complete operator workflow, proves private commands are absent from
  the public surface, and exits nonzero on any failed machine-checkable step.

## Evidence

- Repository inspection -> `scripts/ginux-host-controller.sh` defines the
  public command list with hand-written shell parsing.
- Repository inspection -> `scripts/ginux-host-instance-helper.sh` invokes
  `/usr/local/libexec/ginux-runtime native ... configured` as an internal step
  of `ginux start`.
- Prior Gate 12.25 evidence -> the top-level instance workflow is accepted and
  supplies the regression baseline; this gate re-evaluates its organization
  and implementation rather than assuming every current command is permanent.

## Decisions and limitations

- Rust `clap` is the preferred Cobra-equivalent because Ginux is a Rust
  workspace and the public controller can be a normal host binary. An
  alternative requires an explicit maintenance, dependency, and testing
  rationale at the design checkpoint.
- `native` is not an approved public operator command. The inventory must
  determine whether its internal behavior remains necessary, but its current
  implementation name is not a reason to expose it.
- This gate has two mandatory human checkpoints: command-design approval before
  implementation and final acceptance after verified delivery.
- Pull request 272 may harden the existing internal parser independently; this
  gate owns the broader command architecture and migration.

## Delivery strategy

- Working branch: `hdae/cli-command-framework`
- Intended base: `main`, synchronized after pull request 272 is merged
- Pull request: intended after implementation and verification; not created by
  this planning handoff
- Planned commits: gate and command inventory; approved CLI framework and
  public command model; internal-boundary and installation migration; tests and
  documentation; evidence reconciliation
- Actual commits: pending

## Review policy and outcome

Human checkpoint gate. The implementing agent must first deliver the inventory
and proposed command specification, then stop for human approval before
changing the CLI. After implementation and complete verification, publish one
dedicated pull request and stop again for final human acceptance. Technical
passage alone does not close Gate 25.
