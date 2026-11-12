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
- Give every shipped example the explicit development root password `ginux`,
  mark it `CHANGEME`, and explain that it is not a production credential.
- Boot every shipped example through its distribution's normal init path; no
  example may use Bash or an application directly as PID 1.
- Remove the controller's single-Rocky-image restriction and accept any
  digest-pinned, standards-compliant public Linux OCI root matching the host
  architecture, with an explicit configured init path.
- Key verified image caches by the complete OCI reference and platform so
  unrelated repositories, versions, manifests, and roots cannot collide.
- Add an Ubuntu web-server example with a minimal blank CMS and an Alpine
  example; each must use the distribution's normal init/boot path.
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
  five example recipes, and checkpoint review.
- [x] Created `hdae/readable-instance-recipes` from merged Gate 17 `main`.

## Remaining work

- [x] Implement bounded multiline hook parsing and readable normalization.
- [x] Simplify the Rocky recipe without changing its boot behavior.
- [x] Add and document the five example recipes.
- [x] Make the development root credential explicit and consistent across all
  five examples, with a regression assertion preventing omission.
- [x] Boot every example through its distribution's normal init and verify
  login/readiness under the Rocky QEMU test host.
- [x] Generalize public OCI registry resolution, verification, caching,
  extraction, and instance materialization without weakening digest or layer
  safety checks.
- [x] Add working Ubuntu CMS and Alpine native-boot recipes.
- [x] Verify Rocky, Ubuntu, and Alpine boot plus cache separation and negative
  image/platform/init cases.
- [x] Add deterministic positive, negative, and compatibility tests.
- [x] Run focused and aggregate verification.
- [x] Commit, push, open the Gate 19 pull request, and stop for human review.

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
- Every shipped example sets root's development password to `ginux`, marks it
  `CHANGEME`, and passes the credential-presence regression check.
- Every Rocky and Ubuntu systemd example configures `/sbin/init --system
  --unit=multi-user.target`; the Alpine example uses Alpine's normal native
  init path. No example lands directly in an application shell.
- Import accepts any valid digest-pinned public Linux OCI root for the host
  architecture without repository or distribution allowlists; mutable tags,
  unsupported platforms, malformed descriptors/layers, unsafe paths, digest
  mismatches, and missing configured init fail closed.
- Two different OCI references cannot share prepared roots accidentally, while
  repeated import of the same reference reuses its verified cache.
- Ubuntu boots its normal systemd path and serves a minimal blank CMS; Alpine
  boots its native init path; both expose the documented `ginux` development
  root credential where their login implementation supports it.
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
- `make test-gate19-native-examples` -> each example reaches a console login,
  authenticates as root with its development password, and shuts down cleanly.
- Generic OCI integration -> Rocky, Ubuntu, and Alpine references resolve,
  verify, cache independently, materialize, and boot through their configured
  native init paths; cache-hit reuse is observed.
- Negative OCI integration -> mutable tags, wrong architecture, corrupt
  descriptors/layers, unsafe extraction paths, and missing init are rejected
  without committed instance or cache state.

## Evidence

- `cargo test -p ginux-instance` -> 28 parser, normalization, compatibility,
  rejection, policy, and documented-example tests passed.
- `make demo-gate-19` -> all five documented recipes validated and the focused
  instance tests passed, including the required development root credential.
- `make test-config` -> complete configuration workflow passed under an
  isolated Rocky QEMU host.
- `make test-gate975-hooks` -> ordering, mediation, stop/continue failure,
  timeout, init suppression, and audit behavior passed.
- `make test-systemd` -> cold, warm, repeat, offline, package-failure,
  configured-failure, and controlled-shutdown paths passed with the readable
  Rocky recipe.
- `make test-security` -> malformed input, deterministic fuzz, authority,
  denial, host-view, range, exhaustion, and teardown checks passed.
- `make test-generic-oci` -> digest pinning, cache identity separation, and
  offline prepared-cache reuse passed; mutable tags and short digests failed
  closed.
- `make test-gate19-native-examples` -> the Rocky agent workspace, Ubuntu CMS,
  Rocky paranoid, and Alpine recipes booted their configured native init paths,
  authenticated root with the documented development password, passed their
  workload checks, and shut down cleanly.
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
- "Any OCI root" means a public, digest-pinned Linux OCI image compatible with
  the selected host architecture and Ginux's implemented ABI. It does not
  promise that an image lacking its configured init or required userspace ABI
  will boot, and private-registry credentials remain outside this gate.
- Distribution-native boot is authoritative: systemd for the Rocky/Ubuntu
  examples and Alpine's native init rather than adding systemd to Alpine.
- The Ubuntu CMS example uses the digest-pinned third-party
  `jrei/systemd-ubuntu` image because the official minimal Ubuntu OCI image does
  not include `/sbin/init`.
- Alpine's official minimal image supplies BusyBox init but not OpenRC. Its
  stock inittab may report missing OpenRC hooks before continuing to the native
  BusyBox console login; Ginux does not add a distribution init system.

## Delivery strategy

- Working branch: `hdae/readable-instance-recipes`
- Intended base: `main`
- Pull request: https://github.com/gmkurtzer/ginux/pull/261
- Planned commits: gate and parser; recipes and documentation; evidence and
  pull-request checkpoint
- Actual commits: `fd2c0aba`, `c9079530`, `fa828e70`, `b66bbe72`, and
  `17502f43`, plus the pull-request checkpoint commit.

## Review policy and outcome

Human checkpoint. The human approved implementation and pull-request delivery
on 2026-11-11. Pull request 261 is delivered; stop for review and acceptance.
