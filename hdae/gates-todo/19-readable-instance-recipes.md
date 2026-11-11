# Readable instance recipes and hooks

## Source

Human request after Gate 17 acceptance while Gate 18 awaited review.

## Goal

Make instance recipes readable and useful as operator-facing examples without
moving pre-boot authority outside the portable recipe or weakening mediation.

## Approved scope

- Support ordered multiline basic strings inside `[hooks].pre_boot` arrays.
- Preserve existing single-line hook arrays and normalized round trips.
- Keep every hook inside the instance root under the existing contract,
  timeout, failure, audit, and host-authority rules.
- Rewrite the Rocky systemd recipe into short, logically grouped scripts.
- Add validated minimal Rocky, constrained agent/workspace, networked service,
  and paranoid syscall-control example recipes.
- Document hook syntax, limits, example intent, and relevant CLI workflows.
- Add parser, normalization, rejection, recipe-validation, and regression tests.

## Non-goals

- Host-context hooks, external script dependencies, post-boot hooks, or new
  hook authority.
- Changing the instance lifecycle, contract semantics, network deployment
  model, or Gate 11 performance requirements.
- Making example credentials suitable for production.

## Completed work

- [x] Inspected the existing parser, normalized output, Rocky recipe, hook
  execution boundary, and example documentation.
- [x] Human approved the complete gate scope, dedicated branch, pull request,
  four example recipes, and checkpoint review.
- [x] Created `hdae/readable-instance-recipes` from merged Gate 17 `main`.

## Remaining work

- [x] Implement bounded multiline hook parsing and readable normalization.
- [x] Simplify the Rocky recipe without changing its boot behavior.
- [x] Add and document the four example recipes.
- [x] Add deterministic positive, negative, and compatibility tests.
- [x] Run focused and aggregate verification.
- [ ] Commit, push, open the Gate 19 pull request, and stop for human review.

## Acceptance criteria

- A recipe can contain one or more triple-quoted multiline pre-boot scripts in
  an ordered array, and each script executes as one existing mediated hook.
- Existing one-line hook arrays parse and execute unchanged.
- Normalizing and reparsing either form preserves the effective configuration;
  multiline hooks remain readable rather than becoming one giant line.
- Unterminated strings, invalid trailing content, excessive item counts or
  lengths, NUL/control bytes, relative executables, and host-context hooks fail
  closed.
- The Rocky systemd recipe contains short, logically grouped hook scripts and
  retains its existing bootstrap, lockdown, login, and systemd behavior.
- Minimal Rocky, agent/workspace, networked-service, and paranoid recipes all
  validate, normalize, and document their intended tradeoffs.
- Existing configuration, hook, systemd, security, and aggregate suites remain
  green.

## Verification plan

- `cargo test -p ginux-instance` -> multiline, compatibility, rejection,
  examples, and normalization tests pass.
- `make test-config` -> complete configuration workflow passes.
- `make test-gate975-hooks` -> hooks retain ordering, mediation, failure,
  timeout, and host-authority behavior.
- `make check test-security test-systemd test` -> mandatory regressions pass.
- `make demo-gate-19` -> validates and summarizes every example and exits zero.

## Evidence

- `cargo test -p ginux-instance` -> 27 parser, normalization, compatibility,
  rejection, policy, and documented-example tests passed.
- `make demo-gate-19` -> all four documented recipes validated and the focused
  instance tests passed.
- `make test-config` -> complete configuration workflow passed under an
  isolated Rocky QEMU host.
- `make test-gate975-hooks` -> ordering, mediation, stop/continue failure,
  timeout, init suppression, and audit behavior passed.
- `make test-systemd` -> cold, warm, repeat, offline, package-failure,
  configured-failure, and controlled-shutdown paths passed with the readable
  Rocky recipe.
- `make test-security` -> malformed input, deterministic fuzz, authority,
  denial, host-view, range, exhaustion, and teardown checks passed.
- `make check` and aggregate `make test` -> formatting, lint, unit, image,
  configuration, lifecycle, device, process, virtual-kernel, and QEMU
  integration regressions passed.

## Decisions and limitations

- Multiline strings are recipe content, not paths to host scripts. This keeps
  recipes portable and prevents implicit host authority.
- Existing hook size and count bounds remain explicit and fail closed; any
  capacity change must remain bounded and covered by negative tests.
- Triple-quoted hook bodies preserve their script bytes. Shell escapes such as
  `\n` therefore appear once in the recipe, rather than using the doubled
  escaping required by the legacy one-line string form.
- The network-service recipe documents host attachment and forwarding as CLI
  deployment state rather than embedding host-specific values in the recipe.

## Delivery strategy

- Working branch: `hdae/readable-instance-recipes`
- Intended base: `main`
- Pull request: intended
- Planned commits: gate and parser; recipes and documentation; evidence and
  pull-request checkpoint
- Actual commits: `fd2c0aba`, `c9079530`, `fa828e70`, and `b66bbe72`, plus the
  final evidence checkpoint commit.

## Review policy and outcome

Human checkpoint. The human approved implementation and pull-request delivery
on 2026-11-11. Stop after delivery for review and acceptance.
