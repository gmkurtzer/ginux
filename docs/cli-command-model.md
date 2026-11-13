# Ginux CLI command model

Status: Gate 25 design checkpoint. This document inventories the current
command surfaces and proposes the operator CLI that must be approved before
implementation begins.

## Executable boundary

The installed system currently has three command layers:

1. `/usr/local/bin/ginux` is the public operator command. It is currently
   `scripts/ginux-host-controller.sh` and hand-parses its arguments.
2. `/usr/local/libexec/ginux-instance-helper` is the privileged instance-state
   helper. It is not an operator command, although several of its internal
   verbs currently share names with public commands.
3. `/usr/local/libexec/ginux-runtime` is the freestanding `no_std` workload
   runtime built from `crates/ginux-init`. Its selectors are backend, fixture,
   and component-test entry points, not operator commands.

Gate 25 replaces only the public parser with a normal Rust host binary using
`clap`. It does not link `clap` into the freestanding runtime and does not make
the helper or runtime public.

## Current public command inventory

The following are the complete commands accepted by
`scripts/ginux-host-controller.sh`. “Callers” lists executable repository
callers; README and `docs/` references are documentation rather than callers.

| Command | Current operands and options | Route | Known executable callers | Proposed disposition |
|---|---|---|---|---|
| `create` | `NAME`, repeatable `--set KEY=VALUE`, one `--network ATTACHMENT`, repeatable `--forward SPEC` | privileged helper | `gate12-cli-guest.sh` | retain public |
| `import` | `NAME FILE`, same deployment options as `create` | privileged helper | `gate11-guest-setup.sh`, `gate12-cli-guest.sh`, `gate125-lock-guest.sh`, `test-gate19-native-examples.sh`, `test-systemd-interactive.exp` | retain public |
| `set` | `NAME` and at least one repeatable `--set`, `--network`, or repeatable `--forward` | privileged helper | `gate12-cli-guest.sh` | retain public |
| `edit` | `NAME` | unprivileged editor around privileged export/import | `gate12-cli-guest.sh` | retain public |
| `export` | `NAME` | privileged helper, output on stdout | `gate12-cli-guest.sh` | retain public |
| `list` | no operands | privileged helper | `gate12-cli-guest.sh`, `gate125-lock-guest.sh`, `test-systemd-interactive.exp` | retain public |
| `status` | `NAME` | privileged helper | `gate125-lock-guest.sh`, `test-systemd-interactive.exp` | retain public |
| `start` | `NAME`, flags `--attach` and `--audit` | privileged start followed by unprivileged attach when requested | `gate125-lock-guest.sh`, `expect-gate19-native-login.exp`, `test-systemd-interactive.exp` | retain public |
| `attach` | `NAME` | privileged endpoint authorization followed by unprivileged terminal relay | `test-systemd-interactive.exp` | retain public |
| `stop` | `NAME` | privileged helper | `gate125-lock-guest.sh`, `test-gate19-native-examples.sh`, `test-systemd-interactive.exp` | retain public |
| `lock` | `NAME` | privileged helper | `gate125-lock-guest.sh`, `test-systemd-interactive.exp` | retain public |
| `unlock` | `NAME` | privileged helper | `gate125-lock-guest.sh`, `test-systemd-interactive.exp` | retain public |
| `validate` | `FILE` | runtime `instance-validate` | no public-CLI test; documented workflow only | retain public; move behind normal host-library API |
| `inspect` | `FILE` | runtime `instance-inspect` | no public-CLI test; documented workflow only | retain public; move behind normal host-library API |
| `explain` | `FILE OPERATION` | runtime `instance-explain` | no public-CLI test; documented workflow only | retain public; move behind normal host-library API |
| `diff` | `LEFT RIGHT` | runtime `instance-diff` | no public-CLI test; documented workflow only | remove from the public CLI; retain runtime coverage only where needed by component tests |

The public script also accepts no global options. `--help`, `help`, and
`--version` are currently usage errors. Gate 25 adds generated global and
per-command help, `help [COMMAND]`, and version output.

The public taxonomy remains flat. A redundant `ginux instance ...` group would
reverse the accepted Gate 12.25 interface and add no disambiguation: every
public lifecycle command already operates on an instance. Help output will
group the flat commands under configuration, inspection, lifecycle,
maintenance, and recipe-diagnostic headings.

## Current privileged-helper inventory

This installed `libexec` program is entered only by the compiled public CLI,
its own supervised lifecycle, or controlled maintainer tests. Operators and
documentation must not treat its verbs as supported syntax.

| Helper command | Caller | Classification | Proposed disposition |
|---|---|---|---|
| `create`, `import`, `set`, `export`, `list`, `lock`, `unlock`, `start`, `status`, `stop` | public controller | private privileged protocol | retain behind typed public commands |
| `authorize-attach` | public controller before attach | private privilege transition | retain; fixed arity and numeric identity validation |
| `attach` | public controller after authorization | private unprivileged transport | retain until its terminal relay is moved into Rust |
| `resize` | private attach transport | private privileged terminal operation | retain |
| `prepare-root` | `stage-systemd-instance.sh`, Gate 17 setup | maintainer/test setup | retain private; not installed operator syntax |
| `resolve-network` | `test-gate975-network.sh` | component-test helper | retain test-only |
| `supervise` | helper self-invocation from `start` | private lifecycle protocol | retain |
| `run-runtime` | helper self-invocation from `supervise`; systemd/hook/benchmark probes | private lifecycle protocol | retain |

The helper keeps a strict allowlist and exact arity for every verb. Gate 25
will add a protocol version selected by the compiled CLI and helper
self-invocations so accidental invocation by an incompatible client fails
before state changes. The helper remains the only route to root-owned state.

## Current runtime command inventory

These selectors are accepted by `crates/ginux-init/src/launch.rs`. None is a
public `ginux` command after Gate 25.

| Runtime selector | Known production or test callers | Classification | Proposed disposition |
|---|---|---|---|
| no arguments | `test-intercept.sh`, Gate 1 setup tests | test-only interception fixture | retain test-only |
| `runtime ELF` | `test-runtime.sh` | component test | retain test-only |
| `config FILE` | `test-config.sh` | component test | retain test-only |
| `config-merge FILE OVERRIDE...` | instance helper; Gate 17 setup; `test-config.sh` | private production config service | retain private |
| `instance-plan FILE` | `test-config.sh` | component test | retain test-only |
| `instance-validate FILE` | current public `validate`; `test-launch.py` | transitional public bridge | remove bridge after public host implementation; retain only if a runtime regression still needs it |
| `instance-inspect FILE` | current public `inspect`; `test-launch.py` | transitional public bridge | same as `instance-validate` |
| `instance-explain FILE OPERATION` | current public `explain`; `test-launch.py` | transitional public bridge | same as `instance-validate` |
| `instance-diff LEFT RIGHT` | current public `diff`; `test-launch.py` | transitional public bridge | same as `instance-validate` |
| `hook-plan FILE` | instance helper; `test-launch.py` | private production protocol | retain private |
| `lifecycle FILE` | `test-instance.sh` | component test | retain test-only |
| `vfs FILE ROOTFS INDEX` | `test-vfs.sh` | component test | retain test-only |
| `native FILE ROOT [configured\|hold\|probe]` | instance helper; native VFS, contract, failure, and supervision tests | private production backend plus component modes | retain private; never public |
| `native-hook FILE ROOT INDEX` | instance helper; parser tests | private production backend | retain private; never public |
| `instance FILE FIXTURE` | `test-instance.sh` | component test | retain test-only |
| `process FILE` | `test-process.sh` | component test | retain test-only |
| `process-native FILE FIXTURE` | process/supervision/tracee-kill tests | component test | retain test-only |
| `devices FILE BLOCK` | `test-devices.sh` | component test | retain test-only |
| `device-bench FILE BLOCK KIND` | `bench-devices.sh` | benchmark-only | retain benchmark-only |
| `kernel-views FILE` | `test-kernel-views.sh` | component test | retain test-only |
| `launch FILE ROOT [--result-file PATH]` | `test-launch.py`; maintainer documentation | maintainer/component diagnostic | retain under `ginux-runtime`; remove operator-style `ginux launch` documentation |

The runtime will continue exact-arity validation and fail closed for unknown
selectors. It will not provide operator-oriented help, aliases, completion, or
configuration discovery.

## Other executable surfaces

These are outside the operator CLI and remain explicitly classified:

| Executable | Interface | Classification and caller |
|---|---|---|
| `ginux-vfs-index` | `ROOTFS.tar OUTPUT.index` or `assemble OUTPUT.tar OUTPUT.index LAYER...` | private image-build tool used by OCI preparation and VFS tests |
| `contract-doc` | `--write PATH` or `--check PATH` | maintainer documentation generator used by Make targets |
| `security-fuzzer` | `TARGET SECONDS ARTIFACT_DIRECTORY` | bounded test-only fuzzer used by `fuzz-security.sh` |
| `baseline-runner` | no arguments | benchmark fixture used by baseline targets |
| `managed-payload`, `runtime-fixture`, `process-native-fixture` | fixture-specific arguments | test payloads, never control-plane commands |
| `/usr/local/libexec/ginux-image/*` | script-specific image references and cache/work paths | private image acquisition and preparation tools called by the instance helper or maintainer scripts |

## Proposed public command specification

The compiled `ginux` binary will use `clap` derive declarations as the single
source for parsing, validation, help, and version metadata.

```text
Usage: ginux <COMMAND>

Configure instances:
  create    Create an instance from the installed default recipe
  import    Create or replace an instance from a recipe
  set       Atomically change recipe overrides or host deployment settings
  edit      Edit, validate, and atomically apply an instance recipe
  export    Write an instance recipe to standard output

Inspect instances:
  list      List instances
  status    Show instance state and paths

Run instances:
  start     Start an instance
  attach    Attach to a running instance console
  stop      Stop an instance

Maintain instances:
  lock      Enforce the configured contract for a stopped instance
  unlock    Enable bounded maintenance access for a stopped instance

Inspect recipes:
  validate  Validate and compile a recipe without changing state
  inspect   Show a recipe's normalized effective contract
  explain   Explain one operation's effective contract decision
Options:
  -h, --help       Print help
  -V, --version    Print version
```

Exact command shapes remain:

```text
ginux create NAME [--set KEY=TOML_VALUE]... [--network ATTACHMENT]
                  [--forward SPEC]...
ginux import NAME FILE [--set KEY=TOML_VALUE]... [--network ATTACHMENT]
                       [--forward SPEC]...
ginux set NAME (--set KEY=TOML_VALUE | --network ATTACHMENT |
                --forward SPEC)...
ginux edit NAME
ginux export NAME
ginux list
ginux status NAME
ginux start NAME [--attach] [--audit]
ginux attach NAME
ginux stop NAME
ginux lock NAME
ginux unlock NAME
ginux validate FILE
ginux inspect FILE
ginux explain FILE OPERATION
```

`--set` and `--forward` preserve command-line order. `--network` occurs at
most once. `--forward none` conflicts with every other `--forward` value.
`set` requires at least one change. `NAME`, `FILE`, and `OPERATION` remain
positional. No compatibility alias using `--config` or an `instance` command
group is added.

## Errors, output, and compatibility

- `--help`, `help [COMMAND]`, and `--version` write to stdout and return 0.
- Public parse errors write one concise diagnostic plus relevant usage to
  stderr and return 2. This intentionally standardizes the shell controller's
  current generic exit 64 for malformed public syntax.
- Operational failures preserve the helper or runtime result; Gate 25 does not
  collapse not-found, unavailable, invalid-state, or software-failure results
  into parser errors.
- A rejected invocation performs no sudo call and no state mutation.
- Existing instance recipes, deployment state, image caches, locks, roots,
  console state, and lifecycle semantics are unchanged.
- Existing public command names and successful argument forms remain valid.
  Internal selectors receive no public compatibility aliases.
- Commands not present today—such as `delete`, `restart`, `logs`, image
  management, or shell-completion generation—are outside Gate 25 rather than
  being silently invented during the parser migration.

## Proposed implementation boundary

1. Add a normal host `ginux-cli` Rust binary using `clap`; keep the existing
   freestanding binary installed only as `ginux-runtime`.
2. Put the public command enum, arguments, conflicts, help headings, and
   version in one declarative module tested without root or QEMU.
3. Route recipe diagnostics through reusable host-side library functions, not
   through public knowledge of runtime selectors.
4. Route state-changing commands through the versioned private helper
   protocol. Parse and reject malformed input before invoking sudo.
5. Preserve the current attach privilege transition and terminal behavior;
   move its public orchestration into Rust while leaving only narrow endpoint
   authorization and resize operations in the helper.
6. Install the Rust CLI at `/usr/local/bin/ginux`, the freestanding runtime at
   `/usr/local/libexec/ginux-runtime`, and the private helper under
   `/usr/local/libexec`.
7. Add generated-help snapshots, parser/negative-state tests, command-inventory
   consistency checks, installation assertions, and the existing lifecycle
   and systemd regressions before removing the shell public controller.

## Design decisions requiring approval

Implementation must not begin until the human approves these points:

1. Keep the accepted flat top-level command taxonomy; do not reintroduce
   `ginux instance ...`.
2. Retain every currently successful public command shape, while adding
   generated help/version behavior and standardizing public parse failures to
   exit 2, except for the explicitly removed `diff` command.
3. Keep production backend selectors such as `native`, `native-hook`,
   `config-merge`, and `hook-plan` only in the private runtime protocol.
4. Keep component and benchmark selectors available only through the private
   runtime binary for deterministic tests; do not expose them in public help.
5. Do not add compatibility aliases or new product commands in this gate.
6. Version the private CLI/helper protocol so an incompatible caller fails
   before any privileged state change.

The human approved this design on 2026-11-13 with one correction: remove
`diff` from the public operator CLI.
