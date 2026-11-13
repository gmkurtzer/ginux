# Ginux HDAE Status

## Current checkpoint

### Active implementation: Gate 25 declarative operator CLI

- Gate 25 defines a complete CLI inventory, a human-approved command-design
  checkpoint, migration from the shell controller to a declarative Rust
  command framework, and an explicit private boundary around runtime/backend
  commands such as `native`.
- The dedicated branch is synchronized with current `origin/main`, including
  merged pull request 272.
- `docs/cli-command-model.md` inventories the public controller, privileged
  helper, freestanding runtime, test/benchmark tools, and their known callers.
  It proposes a flat top-level operator taxonomy, a versioned private helper
  protocol, generated help/version behavior, and the disposition of every
  runtime selector.
- The human approved the command design on 2026-11-13 with one correction:
  remove `diff` from the public CLI. Implementation may now proceed within the
  approved Gate 25 boundary.
- Fresh baseline evidence: `make check` passes on the synchronized branch.
- Design checkpoint commit: `5b315d3e`.
- Active skill: `hdae-development`
- Skill path: `.agents/skills/hdae-development/SKILL.md`
- Reload after context shift: required
- Exact next action: implement the approved Rust `clap` command model without
  public `diff`, then add parser, installation, and workflow verification.

- Gate 1 is accepted and closed under the corrected ordinary-host-process
  architecture. Pull request https://github.com/gmkurtzer/ginux-us/pull/1 was
  merged by the human on 2026-11-04.
- Gate 2 is accepted and closed. Pull request
  https://github.com/gmkurtzer/ginux-us/pull/2 was merged by the human on
  2026-11-04.
- Gate 3 is accepted and closed. Pull request
  https://github.com/gmkurtzer/ginux-us/pull/3 was merged by the human on
  2026-11-04.
- Gate 4 is accepted and closed. Pull request
  https://github.com/gmkurtzer/ginux-us/pull/4 was merged by the human on
  2026-11-04.
- Gate 5 is accepted and closed. Pull request
  https://github.com/gmkurtzer/ginux-us/pull/5 was merged by the human on
  2026-11-04.
- Gate 6 is accepted and closed. Pull request
  https://github.com/gmkurtzer/ginux-us/pull/6 was merged by the human on
  2026-11-04.
- Gates 7, 7.5, and 7.75 are accepted and closed. Their pull requests were
  merged by the human on 2026-11-04.
- Gate 8 is accepted and closed. Pull request
  https://github.com/gmkurtzer/ginux-us/pull/10 was merged as `2306c2e`, and
  the human confirmed the gate complete on 2026-11-04.
- Authoritative name: Ginux.
- Runtime isolation unit: instance.
- Policy unit: versioned contract.
- The gate map has expanded through human-directed insertions while preserving
  numeric order. Gates 1 through 10.5, 12, 12.1, 12.25, 12.5, 13, and 15 are
  accepted and closed. Gate 11 remains partial because its unchanged 4%
  native-performance budget is not met by selective thread-yield. Gate 14
  remains explicitly deferred.
- Gate 9.75 was added and authorized on 2026-11-04 for comprehensive instance
  configuration, CLI overrides, network selection and policy, port translation,
  volumes, access, identity, resources, devices, and pre-boot hooks.
- Gate 9.75 was accepted and closed by the human on 2026-11-06. Pull request
  https://github.com/gmkurtzer/ginux/pull/229 was merged as `8a475e8b`.
- Gate 10 is accepted and closed on `hdae/configurable-contract`. The versioned
  catalog contains 182 explicit operations;
  normalized contracts produce immutable digests and provenance; semantic and
  syscall decisions are enforced before host effects; and audit detail is
  bounded to 8,192 records with logarithmic dropped-event accounting.
- Gate 10 pull request https://github.com/gmkurtzer/ginux/pull/231 was accepted
  and merged by the human as `ff926d0c` on 2026-11-06.
- Fresh Gate 10 evidence passed `make test-contract`, `make check`,
  `make test-contract-runtime`, `make test-systemd`, the Gate 9.75 network and
  hook regressions, aggregate `make test`, and `make demo-gate-10`. The demo
  showed strict/systemd plan differences, a real socket denial with no host
  mutation, the audit bound, dropped-event accounting, and controlled poweroff.
- Gate 9.75 now has a normalized typed schema and CLI override path plus passing
  Rocky QEMU evidence for interface/source selection, ingress/egress policy,
  translated port ownership, ordinary bind conflicts, network audit records,
  payload exchange, and cleanup. Its instance-only pre-boot hooks now also pass
  ordered execution, mediation, mount denial, stop/continue failure handling,
  timeout, init suppression, and audit tests under Rocky QEMU.
- The combined `make demo-gate-9.75` checkpoint passes the complete schema and
  negative validation matrix, source/interface resolution, network policy and
  translation, conflicts and cleanup, and mediated hook behavior. Strict
  `make check`, the complete `make test-systemd` matrix, and aggregate
  `make test` also pass.
- Gate 11 requires a matched native-Rocky versus Ginux `sysbench` matrix,
  native parity as the optimization goal, a default 4% per-workload tolerance,
  and automatic failure for regressions beyond the approved native or
  accepted-Ginux baseline. Its unoptimized 1/2/4-vCPU checkpoint is complete:
  all CPU and memory rows pass, while all thread-yield and mutex rows fail.
  The human approved that baseline on 2026-11-07; profiling and optimization
  are now active with the default 4% budget unchanged.
- Gates 9.75 and 10 were clarified on 2026-11-06 to support versioned default
  contract templates, bounded glob and resource selectors, deterministic TOML
  overrides, and expansion against a pinned operation catalog. Gate 10 must
  also produce a generated ABI coverage document that tracks implemented,
  partial, denied, planned, and excluded kernel/userspace interfaces without
  allowing catalog growth to broaden an old contract silently.
- Gates 9 and 9.5 passed their automated and interactive evidence and were
  accepted by the human on 2026-11-06. Their complete implementation reached
  `main` through signed merge commit `a8b46d21` and is now closed.
- Gate 9.5 reuses the writable Rocky host overlay across
  `make host-run` sessions, preserving verified OCI image content, instance
  definitions, roots, and status, and reports live layer-download progress.
  Synthetic proc refresh is now path-targeted rather than rebuilding the whole
  proc tree for each open. In a clean QEMU run, `ps -ef` fell from the human's
  1.871-second reproduction to 309 ms, a single `top` refresh took 913 ms, and
  Bash reported valid child user/system timing. The complete interactive,
  systemd, and aggregate test matrices pass.
- Gate 10.25 was accepted and closed after pull request
  https://github.com/gmkurtzer/ginux/pull/235 merged as `59e2f7aa`. The active product now uses `instance`
  consistently across CLI, schema, Rust APIs, persistence, audit, tests, and
  documentation; there are no compatibility aliases. Static checks reject
  stale terminology, and the legacy CLI noun and TOML table fail closed.
- Fresh Gate 10.25 evidence passed `make check`, `make test-contract`,
  `make test-unit`, a clean aggregate `make test`, `make test-systemd`, Gate
  9.75 network and hook regressions, and `make demo-gate-10.25`. The demo
  booted Rocky systemd, exercised the complete instance lifecycle, verified
  `/var/lib/ginux/instances`, and returned cleanly to the host.
- Normal `ginux instance create` output now separates operator presentation
  from low-level OCI diagnostics. Fresh-cache creation reports one concise
  image summary, a bounded live progress bar for each layer, verification, and
  preparation; cache reuse omits downloads and reports the hit. Detailed
  digests and internal paths remain available to standalone diagnostic mode.
- Fresh output evidence passed `make check`, all seven `make test-oci-image`
  checks, and `ROCKY_SSH_PORT=22410 make test-systemd-interactive`, including
  cache miss, offline cache hit, persistent reuse, Rocky systemd boot, and the
  complete instance lifecycle while the human's separate test VM remained
  untouched.
- Attached instance consoles now inherit the host terminal geometry and track
  `SIGWINCH` changes. Fresh interactive evidence passed at `37x101`, opened and
  exited `vi` without corrupting the size, then detached and reattached at
  `43x109`; `make check` and the complete Rocky interactive workflow passed.
- Native child-ID translation no longer injects a software breakpoint into
  shared workload text. An isolated interactive Rocky regression completed
  `dnf -y install vim` without the reported unexpected trace stop or instance
  teardown. Socket bookkeeping exhaustion is now operation-local (`EMFILE`),
  `close_range` releases tracked sockets, and the remaining interactive
  lifecycle, restart, and persistent-cache checks passed.
- Gate 10.5 was accepted by the human on 2026-11-07 after pull request
  https://github.com/gmkurtzer/ginux/pull/237 merged as `53a54add`. Its
  self-contained contract UX, recursive filesystem policy, recipe-owned
  bootstrap, set-ID/device-node controls, reduced host identity, and
  `NoNewPrivs` backstop are closed with passing pull-request CI.
- Gate 11 began on dedicated branch `hdae/performance-characterization` from
  merged `main`. The first checkpoint is the unoptimized matched native Rocky
  versus Ginux baseline; no optimization begins before that evidence is
  presented.
- The ordered gate batch is authorized, but every open gate remains a human
  checkpoint requiring a working demonstration and explicit acceptance.
- Gate 12 is technically passed on `hdae/contract-network-ux`. The active
  configuration now uses `[contract]` plus `syscalls`, `filesystem`, `network`,
  `devices`, `privileges`, `process`, and synthetic `kernel` subsections.
  Syscall globs compile against catalog version 2, unmatched globs fail closed,
  and explicit deny wins.
- Gate 12 host networking is deployment state rather than portable recipe
  content. Both `instance create` and `instance set` accept repeatable
  `--network` and `--forward` flags, validate the complete candidate, and
  commit atomically. Rocky integration proves simultaneous forwards, default
  attachment, recipe/deployment separation, and unchanged state after an
  invalid multi-flag update.
- Fresh Gate 12 evidence passed `make check`, `make test-contract`,
  `make test-config`, `make test-instance`, `make test-contract-runtime`,
  `make test-network`, the complete `make test-systemd` matrix, aggregate
  `make test`, and `make demo-gate-12`.
- Gate 12.25 is technically passed on `hdae/top-level-instance-cli`. Ginux now
  manages instances through concise top-level `create`, `import`, `set`,
  `edit`, `export`, inspection, and lifecycle commands; the removed command
  group has no compatibility alias. Default creation, atomic recipe updates,
  deployment-state separation, full Rocky systemd behavior, and aggregate
  regressions pass.
- Gate 12.5 is technically passed on `hdae/contract-maintenance-lock`.
  Instances default fail-closed to a persistent locked contract; top-level
  `lock` and `unlock` require a stopped instance, preserve the portable recipe,
  materialize a bounded maintenance contract atomically, expose their state in
  lifecycle output, and warn on unlocked starts. A clean Rocky workflow proved
  denial while locked, success while unlocked, and denial again after relock.
- Gate 13 boundary hardening was accepted and closed on 2026-11-10 on
  `hdae/boundary-hardening`. Its deterministic malformed-input and fuzz suite,
  post-bootstrap supervisor authority reduction, security and recovery suites,
  aggregate regressions, limitations report, and human demonstration pass.
  A fresh matched comparison against current `main` passes every accepted-Ginux
  regression row within 4%; the worst ratio is 0.9718x and thread-yield ranges
  from 0.9760x to 1.0175x. Gate 11 separately retains ownership of its
  unresolved native-parity target. Post-review lifecycle cleanup now reserves
  interactive-host destruction for explicit `make host-reset`, automated QEMU
  tests use separate state, and audit recipes use explicit `enabled` booleans
  with a controller-owned log destination. The rebased result is delivered in
  pull request https://github.com/gmkurtzer/ginux/pull/255. A subsequent
  terminal fix tracks duplicated console descriptors
  and the virtual foreground process group. Commit `20c036dd` keeps sudo out of
  the live relay, preserves the runtime control descriptor, and makes the
  attach process own input for its full lifetime. The complete clean Rocky
  workflow now proves Bash monitor mode, foreground Ctrl-C interruption,
  foreground Ctrl-Z suspension with job tracking, Ctrl-] detach after either
  signal, pipelines, background jobs, prompt-time background completion,
  reattach, resizing, and shutdown without terminal process-group or
  no-job-control warnings. `make check`,
  `make test-host-lifecycle`, aggregate `make test`, and the full
  `make test-systemd-interactive` workflow pass.
- Gate 14 is parked by human direction and has no active implementation work.
- Gate 15 is technically complete on `hdae/dispatch-performance-matrix` and is
  delivered for human review in pull request
  https://github.com/gmkurtzer/ginux/pull/257. Checked-in suite, mode, and per-workload templates
  drive a paired native/selective/paranoid matrix at 1/2/4 vCPUs. The workflow
  preserves raw samples and provenance, validates compatible metadata, and
  produces combined TSV, Markdown tables with mean and standard deviation, and
  a native-relative SVG graph.
- The full Gate 15 run collected 400 measured samples. Selective CPU, memory,
  and mutex rows remain between `0.9642x` and `1.0235x` native; selective
  thread-yield is `0.9216x` to `0.9401x`. Paranoid CPU, memory, and mutex rows
  remain between `0.9618x` and `1.0417x`, while all-syscall mediation makes
  paranoid thread-yield `0.0275x` to `0.0350x`. Gate 15 reports these costs;
  Gate 11 retains responsibility for any further optimization.
- Gate 15 verification passes `make test-gate15-report`,
  `make demo-gate-15`, the full `make bench-modes` matrix,
  `make bench-modes-report`, `make check`, `make test-contract`,
  `make test-security`, `make test-systemd`, and aggregate `make test` including
  all QEMU integrations.
- Pull requests 251, 252, 253, and 254 merged the accepted Gate 12, 12.1,
  12.25, and 12.5 work into `main` on 2026-11-09. Pull request 257 merged the
  accepted Gate 15 work on 2026-11-11. Their authoritative records now reside
  in `hdae/gates-done/`.
- Gate 16's kernel-module enforcement direction was abandoned by human
  decision on 2026-11-11. Pull request 258 was closed without merging, and no
  Gate 16 implementation is present on `main`.
- Gate 17 concurrent-instance isolation was accepted after pull request 259
  merged on 2026-11-11. Its authoritative record is in `gates-done/`.
- Gates 18 and 19 were explicitly accepted and closed by the human on
  2026-11-12. Pull requests 260, 261, and the Gate 18 documentation-check
  follow-up 262 are merged; their authoritative records are in `gates-done/`.
- Gates 20, 21, and 22 were explicitly accepted and closed by the human on
  2026-11-12. Pull requests 263 and 264 are merged, and pull request 265 is the
  accepted Gate 22 delivery; their authoritative records are in `gates-done/`.

## Governing outcome and invariants

A normal Rocky Linux 10 host must boot with systemd as real PID 1. Ginux runs
as an ordinary host process and launches an unmodified Rocky Linux 10 systemd
userspace as virtual instance PID 1 without allowing managed code to invoke host
syscalls directly. All managed interactions are emulated, mediated, or denied
according to the instance contract. Instance-visible kernel state and devices are
Ginux-owned.

See `hdae/PROJECT.md` and `hdae/ARCHITECTURE.md` for the authoritative intent,
scope, threat model, and invariants.

## Resumption

- Active skill: `hdae-development`
- Skill path: `.agents/skills/hdae-development/SKILL.md`
- Reload after context shift: required
- Active gate: none. Gates 18 and 19 are accepted and closed. Gate 11 remains
  partial, Gate 14 is deferred, and Gate 16 is abandoned.
- Last evidence: `make test-generic-oci`, `make demo-gate-19`,
  `make test-gate19-native-examples`, `make check`, `make test-config`,
  `make test-gate975-hooks`, `make test-security`, `make test-systemd`, and
  aggregate `make test` passed. Rocky and Ubuntu reached systemd
  `multi-user.target`; Alpine reached its BusyBox init console login.
- Exact next action: obtain human direction on whether to resume or defer the
  partial Gate 11 performance target. Gate 14 remains deferred.
- The final CLI correction makes required configuration files positional:
  `instance create NAME FILE`, `validate FILE`, `inspect FILE`, and
  `explain FILE OPERATION`. Fresh `make check`, `make test-contract`, and
  isolated Rocky QEMU `make test-config` pass. The interactive Rocky workflow
  also rejected legacy create syntax and completed positional creation, cache
  reuse, and systemd boot before its later proc identity check timed out.
- The expanded Gate 10.5 implementation now passes strict formatting/build,
  all 21 instance tests, `make check`, `make test-contract`,
  `make test-contract-runtime`, `make test-process`, `make test-systemd`,
  aggregate `make test`, and `make demo-gate-10.5`. A clean isolated Rocky
  interactive run additionally proves recipe-owned bootstrap; recursive,
  symlink-safe, and link-creation-safe path denial; set-ID and device-node
  denial with FIFO allowance; root-origin host UID/GID 65534 and
  `NoNewPrivs: 1`; and the complete attach, shutdown, restart, and cache flow.
- Pull-request CI found that the Gate 1 setup-failure fixture's private
  root-owned mode-0700 `/var/tmp` bind prevented the newly reduced host identity
  from reaching its payload. The fixture now gives that private mount to host
  UID/GID 65534; the production pre-exec identity reduction is unchanged.
  Pull-request `launch-integration` now passes in 33 seconds, and
  `static-and-unit` passes in 41 seconds.
- Open decision: no workload-specific tolerance exceptions are approved; the
  default 4% per-row Gate 11 budget remains authoritative. Further work on
  that gap requires an explicitly approved approach that does not reintroduce
  the abandoned kernel-module direction.
- Gate 11 profiling corrected the mutex protocol; every exploratory 1/2/4-vCPU
  mutex row now passes within 0.7% of native. The retained seccomp user
  notification path improved one-vCPU thread yield from 3.299 to 19.731
  events/s while preserving pre-effect userspace dispatch and bounded audit.
  The clean repeated 1/2/4-vCPU matrix remains only 0.031x to 0.036x native and fails
  the unchanged 4% budget.
- Human clarification requires Gate 11 to continue through exhaustive
  implementation-level profiling and optimization rather than stopping at the
  first ptrace-bound result. No workload-specific tolerance exception is
  approved.
- The thread-yield hot path is now exhausted within the approved architecture.
  Audit-disabled user notification reaches 21.789 versus 645.097 native;
  receive plus response ioctl round trips dominate. A removed, exact-semantics
  kernel-resident seccomp decision reached 0.937x native, while a removed
  no-yield diagnostic reached 2.990x and was rejected as semantically invalid.
- Clean-tree Gate 11 metadata identifies commit `6de49677`. Every CPU and mutex
  row passes. A short, high-variance 2-vCPU/1-thread memory failure was not
  accepted at face value: a five-second, five-repetition confirmation passed at
  1.003x native with 1.52% Ginux variance; its 2-thread companion also passed.
- Human direction on 2026-11-07 authorizes an extended Gate 11 selective-mode
  optimization sweep without another intermediate checkpoint. Profile every
  mandatory workload, implement only evidence-backed optimizations, rerun the
  complete 1/2/4-vCPU matrix after material changes, and preserve all contract,
  denial, compatibility, and security invariants. Continue for up to ten hours,
  until all mandatory rows pass, or until every credible in-scope path has
  produced conclusive evidence and any remaining option requires a new
  architecture decision.
- The extended sweep is complete. Per-workload audit capture, scheduler and
  `perf` counters, a direct-futex diagnostic, shared and dedicated supervisor
  CPU placement, audit-disabled execution, and a no-tracer constant-allow
  seccomp control exhaust the credible in-scope paths. The no-tracer control
  alone measured 0.941x native, reproducing the residual thread deficit in the
  host kernel's seccomp fast path under x86-64 QEMU TCG.
- Human direction on 2026-11-07 extends Gate 11 to paranoid mode. The next
  phase will collect a matched paranoid matrix, profile the notification
  broker and its audit, contract, PID-map, and scheduling costs, and test which
  selective-mode optimizations transfer without weakening the requirement that
  every syscall enter Ginux userspace before completion.
- The paranoid follow-on sweep is complete. Its five-second/five-repetition
  matrix passes every CPU, memory, and mutex row but fails all five
  thread-yield rows at 0.028x to 0.037x native. Audit-disabled, CPU-separated,
  and multi-broker diagnostics confirmed that seccomp notification
  receive/response and scheduler round trips dominate; multiple receivers made
  performance substantially worse and were reverted.
- The authoritative final retained-code five-second/five-repetition matrix at
  clean commit `190a625c` passes every CPU, memory, and mutex row. All five
  selective thread rows fail at 0.917x to 0.953x native. The unchanged 4%
  budget therefore keeps Gate 11 partial.
- Same-harness controls prove why: detaching Ginux while retaining one
  constant-allow seccomp filter still measured 0.930x to 0.952x, while
  detaching with no seccomp filter returned stable four-vCPU controls to
  1.003x and 0.980x native. `sched_yield` is already direct-safe and allowing
  observed non-PI futexes did not materially improve the result. The residual
  selective deficit is the host seccomp/task-work floor under QEMU TCG, not a
  missed Ginux allow rule or userspace hot path.
- Exact next action: implement persistent fail-closed `lock`/`unlock` state,
  bounded maintenance materialization, visibility, warnings, and focused
  regressions. Do not merge or close any pull request. Gate 11 remains partial
  in pull request 249 beneath this stacked series.
- Human direction on 2026-11-07 resolved the Gate 11 dispatch boundary. Each
  instance will configure `paranoid` or `selective` dispatch. Paranoid preserves
  a Ginux userspace decision for every syscall. Selective directly executes
  only explicitly allowed operations independently cataloged as transparent
  and direct-safe; denied and complex operations still enter Ginux, and Ginux
  returns configured denials without invoking the requested host syscall.
- Human direction later on 2026-11-07 made selective the default for omitted
  dispatch configuration. Paranoid remains an explicit all-syscall mediation
  option, and the direct-safe catalog remains independently bounded.

## Evidence

- Repository inspection before establishment: clean `main` branch with no
  pre-existing root `hdae/` project ledger.
- Human direction confirmed the Ginux name, instance terminology, Rust/x86-64
  direction, Rocky Linux 10 systemd baseline, cached cloud QCOW2 source,
  virtual kernel trees, device assignment requirement, benchmark requirement,
  and dedicated branch/PR delivery per implementation gate.
- Human direction on 2026-11-03 selected macOS arm64 plus QEMU TCG as the
  development environment, removed KVM from initial acceptance, authorized the
  ordered gate batch, required a runnable human demonstration for every
  checkpoint, and authorized pushing dedicated branches and opening PRs.
- Human direction on 2026-11-03 removed the public-exploit demonstration gate.
  The project retains deterministic defensive contract, malformed-input,
  escape-attempt, fuzz, recovery, and authority-reduction verification.
- Human direction on 2026-11-03 corrected the host boundary: Rocky Linux boots
  normally with host systemd as PID 1; Ginux is an ordinary process or service
  that creates and supervises instances beneath it. Instance systemd is virtual
  PID 1. The earlier Gate 1 PID-1 result is retained as superseded evidence,
  not accepted product behavior.
- Coverage review: functionality, failure behavior, deterministic tests, user
  configuration, documentation, compatibility, recovery, performance, and
  security are assigned to explicit gates; none is silently omitted.
- Revised Gate 1 clean verification on 2026-11-03 passed `make doctor`,
  `make host-image-verify`, `make build`, `make check`, `make test-unit`,
  `make test-intercept`, `make test`, `make host-boot-test`,
  `make bench-baseline`, and `make demo-gate-1` under QEMU TCG.
- Rocky Linux 10.2 booted normally with host systemd as real PID 1. Ginux ran
  as the unprivileged ordinary `ginux` process, mediated the fixed payload,
  exited zero, and left systemd plus `multi-user.target` healthy.
- The revised demo showed virtual `getpid` 4242, checked write mediation without
  host execution, `uname` denied with `EPERM`, no direct-write marker, matched
  Rocky-host benchmark samples, clean QEMU shutdown, and no remaining PID file
  or VM process.
- Human accepted Gate 1 on 2026-11-03 and directed advancement to Gate 2.
- Final Gate 1 verification after removing Ginux numeric host-PID checks passed
  `make check`, `make test-unit`, and `make demo-gate-1`; normal host PID 1
  validation remains only in the Rocky boot harness.
- Gate 2 `make image-prepare`, `make test-image`, `make demo-gate-2`, clean-cache
  preservation checks, and aggregate `make test` passed. The pinned helper
  mounted the source XFS root read-only with networking disabled; repeat
  extraction produced manifest SHA-256
  `49e69bc6209b9d8ed084e221924b633c3126ccaa9a2f920bb8464249a5f66721`;
  corrupt and missing sources were rejected.
- After human feedback that the original demo was opaque, the revised
  `make demo-gate-2` reported eight named checks and their concrete values,
  asserted a read-only source mount plus no helper Ethernet/default route, and
  explicitly stated that Ginux execution, VFS, and systemd boot are not Gate 2
  evidence. The revised demonstration passed on 2026-11-03.
- Human accepted Gate 2 on 2026-11-03 and directed advancement to Gate 3.
- Gate 3 implemented the security-focused x86-64 interpreter, Ginux-owned
  memory/permissions, bounded static ELF loader, Linux initial stack, central
  syscall events, output mediation, audit records, and fail-closed errors.
  Six runtime unit tests, `make test-runtime`, `make demo-gate-3`, and aggregate
  `make test` passed. The interpreted fixture emitted its argument/environment,
  syscall 999 returned `-ENOSYS`, virtual `getpid` returned 4242, exit returned
  zero, and a truncated ELF was rejected before execution.
- Human accepted Gate 3 on 2026-11-03.
- Human authorized Gate 4 on 2026-11-03 and approved strict, versioned TOML as
  its operator-facing configuration format.
- Gate 4 implemented strict versioned TOML parsing and normalization, complete
  fail-closed validation, immutable effective configuration, a single-instance
  lifecycle, managed fixture execution, failure cleanup, signal-driven stop,
  and helper reaping. Seven instance unit tests, `make test-config`,
  `make test-instance`, `make demo-gate-4`, and aggregate `make test` passed.
- Human accepted Gate 4 on 2026-11-03 and directed advancement to Gate 5.
- Human corrected and approved the Gate 5 backing strategy on 2026-11-03: the
  instance virtually boots a digest-pinned Rocky OCI image through Ginux. Ginux
  explicitly pulls and caches the image, assembles its layers into an immutable
  indexed root, and supplies a bounded in-memory write overlay. An external OCI
  runtime must not execute managed code, and the host QCOW2 remains independent.
- Gate 5 now has a verified pinned OCI acquisition/cache pipeline and a Ginux
  immutable root index. `make test-oci-image` passed six acquisition,
  integrity, identity, corruption, repeatability, and offline checks.
- `make test-vfs` passed in the QEMU TCG Rocky host. Ginux resolved Rocky
  identity and Bash from the OCI root, created and discarded its own overlay
  file, kept the host filesystem unchanged, and rejected a truncated index.
- Gate 5 native execution milestone commit `8e5847a` added the `ptrace-native`
  backend. `make check`, `make test-unit`, and `make test-native-vfs` passed.
  The QEMU Rocky host loaded the unmodified OCI Bash and image loader, Ginux
  audited 238 syscall entries, emulated `getpid` as 4242, denied unsupported
  calls, printed `GINUX_NATIVE_SHELL: PASS`, stopped at `exit_group`, and
  preserved the Bash digest.
- Gate 5 multi-layer assembly commit `19ade45` removed the one-layer-only
  preparation restriction. `make test-oci-layers` and the expanded seven-check
  `make test-oci-image` passed replacement, whiteout, opaque-directory,
  repeatability, integrity, identity, and offline checks. A fresh
  `make test-native-vfs` passed against the resulting assembled root.
- The expanded native QEMU test also passed fail-closed supervisor loss: after
  a held Rocky Bash reported readiness, killing Ginux caused
  `PTRACE_O_EXITKILL` to remove the tracee. Normal execution, emulated `getpid`,
  denied `getppid`, immutable Bash content, and cleanup passed in the same run.
- Human direction on 2026-11-03 approved a balanced execution profile for Gate
  5: native instruction execution with minimal confined passthrough for
  read-only loader/image, memory, signal, and bootstrap-console operations;
  sensitive interfaces remain emulated, proxied, or denied. The security
  profile retains interpreted execution and Ginux-virtual descriptors.
- The approved balanced model is now executable. Local checks passed explicit
  syscall-surface and negative policy tests. QEMU passed Rocky Bash with
  classified read-only image and console passthrough, virtual identity, denied
  ioctl, blocked `O_CREAT` with no file created, blocked socket creation, and
  fail-closed supervisor loss.
- The latest native QEMU run also passed unexpected-stop handling: after an
  external `SIGSTOP`, Ginux killed and reaped the tracee and cleaned instance
  state. Four VFS unit tests now include independent overlay-instance isolation
  plus traversal, symlink-loop, invalid-descriptor, overlay, and teardown paths.
- `make demo-gate-5` passed all eight named checks and reported its explicit
  non-claims. A fresh aggregate `make test` also passed all earlier-gate lint,
  unit, QEMU interception/runtime/config/instance, and image-pipeline regressions.
- Gate 5 is delivered in open pull request
  https://github.com/gmkurtzer/ginux-us/pull/5 from dedicated branch
  `hdae/virtual-vfs-userspace`. It is not merged or closed.
- Human accepted Gate 5 on 2026-11-03 and directed advancement to Gate 6.
- Gate 6 began on `hdae/device-ownership`. The first milestone added the
  bounded Ginux device registry, virtual descriptor namespace, assignment
  visibility, exclusive ownership and revocation, console/null/zero/random
  frontends, fail-closed ioctls and mappings, bounded partitions, and the
  isolated packet-queue model.
- `cargo test -p ginux-devices`, its strict clippy check, and the initial
  component targets passed. `make test-devices` then passed end-to-end in the
  Rocky QEMU host with normalized assignment, a disposable file-backed bounded
  partition, isolated packet exchange, denial checks, flush, revocation, and
  backend reuse.
- `make bench-block` and `make bench-network` passed in Rocky x86-64 under QEMU
  TCG and recorded workload, repetitions, bytes, wall time, process CPU time,
  throughput, latency, and environment without imposing a budget.
- `make demo-gate-6` passed all 6 named checks. A fresh `make test` passed
  formatting, strict lint, 31 unit tests, prior-gate regressions, Gate 2 image
  checks, and the Gate 6 Rocky QEMU device path.
- Gate 6 is delivered in open pull request
  https://github.com/gmkurtzer/ginux-us/pull/6 from dedicated branch
  `hdae/device-ownership`. It is not merged or closed.
- Human accepted Gate 6 on 2026-11-03 and directed advancement to Gate 7.
- Human direction added usable multi-user login, virtual PID inspection,
  lockdown checks, and a DNF update workflow to the Gate 9 systemd milestone.
  The DNF repository source remains an explicit decision required before Gate
  9 begins and does not block Gate 7.
- Gate 7 began on `hdae/process-runtime`. Its first milestone added a bounded
  Ginux process table with virtual lifecycle and identity, generation-safe task
  handles, virtual credentials and capability dropping, signal masks/queues,
  futex and eventfd primitives, deterministic exhaustion, and shutdown cleanup.
- The initial `cargo test -p ginux-process`, strict clippy, and
  `make test-process` run passed six component tests. Native supervisor
  integration, stress, QEMU fixtures, aggregate regression, and the demo remain
  incomplete.
- The second Gate 7 milestone added signal dispositions and exec reset,
  monotonic/realtime clocks, one-shot and periodic timers, and bounded
  epoll-style event readiness. Seven process-component tests and strict clippy
  passed; native integration and interruption semantics remain incomplete.
- The Rocky QEMU `make test-process` fixture passed 256 repeated virtual
  lifecycle cycles plus signals, virtual credentials, futex, eventfd, epoll,
  timers, stale identity denial, and zero-task cleanup. This fixture currently
  validates the Ginux process model.
- Gate 7 native supervision now auto-attaches forked tracees and translates
  `fork`, `getpid`, `getppid`, and `wait4` across host and virtual identities.
  The Rocky QEMU stress passed 32 fork/wait cycles with two concurrent children
  per round, 32 expected child exits, interrupted-wait restart handling, and
  zero-task cleanup.
- Gate 7 stress found and fixed raw `PTRACE_PEEKTEXT` ABI handling, the
  auto-attached child's initial trace phase, and host-PID argument restoration
  across an interrupted/restarted `wait4`.
- Fresh `make test-runtime`, `make test-native-vfs`, `make demo-gate-7`, and
  aggregate `make test` passed. The demo reports nine named checks; the native
  regression retains fail-closed supervisor-loss and unexpected-stop evidence.
- Gate 7 is technically passed. Pull request
  https://github.com/gmkurtzer/ginux-us/pull/7 was merged by the human on
  2026-11-04, but the gate still awaits explicit human checkpoint acceptance.
- Gate 7.5 completed the transitional active-project rename, selected `ginux`
  as the executable, and selected `ginux-*` package names; its deterministic
  stale-name check passed.
- Post-rename `make check`, `make test`, `make test-native-vfs`, and
  `make demo-gate-7` passed. The demonstration again passed all nine checks.
- The first aggregate run found a predecessor generated rootfs manifest in the
  persistent cache. The cache now rejects that marker and rebuilds it; the
  image suite and full aggregate suite passed after the migration fix.
- The transitional GitHub repository rename preserved PRs 1 through 7 as
  reachable merged records. Gate 7.5 remains delivered in open
  https://github.com/gmkurtzer/ginux-us/pull/8 through repository redirects.
- Human direction on 2026-11-04 selected Ginux as the final project and
  repository name because the full kernel project is pivoting to this
  userspace-kernel architecture. The local checkout directory must not move.
- Gate 7.75 active surfaces and demonstrations now use Ginux. `make check`,
  `make test`, `make test-native-vfs`, and `make demo-gate-7` passed; the
  demonstration again passed all nine process-runtime checks.
- Gate 7.75 is delivered in open
  https://github.com/gmkurtzer/ginux-us/pull/9 from `hdae/ginux-rename`.
  GitHub rejected renaming the repository to `gmkurtzer/ginux` with HTTP 422
  because that repository already exists and is non-empty.
- Human direction on 2026-11-04 accepted Gates 7, 7.5, and 7.75 and directed
  advancement to Gate 8. Repository integration into the existing Ginux project
  is deferred until the separately planned move on the following morning.
- Gate 8 began on `hdae/virtual-kernel-views`. Its first milestone added the
  no-std kernel-view component, bounded process snapshots, virtual procfs task,
  memory, uptime, mount, hostname, and boot-identity reads, explicit read-only
  mutation denial, and host-PID exclusion tests.
- Gate 8 virtual sysfs now projects only assigned devices with virtual
  class/type/major/minor/uevent data and virtual CPU inventory. Unassigned
  host-device lookup and all sysfs mutation fail explicitly.
- Gate 8 now also has a bounded cgroup v2 hierarchy and instance-owned machine,
  boot, hostname, mount, PID, UTS, and cgroup namespace identities. Six
  component tests cover positive systemd-style operations and negative
  privilege, mutation, capacity, traversal, host-PID, and device leakage paths.
- Gate 8 Rocky QEMU integration passed virtual procfs, sysfs, cgroup, and
  identity markers while host cgroup, mount, and block inventories remained
  byte-identical. `make demo-gate-8` passed all seven named checks.
- Fresh aggregate `make test` passed 44 component tests, all prior-gate QEMU
  regressions, and the Gate 8 QEMU fixture.
- Gate 8 was merged through https://github.com/gmkurtzer/ginux-us/pull/10 as
  `2306c2e`; the human accepted its demonstration and evidence on 2026-11-04.
- Both repositories were fetched after that merge and verified to have no open
  pull requests or issues. The final standalone-kernel commit is `4d4a53bb`.
- Gate 9 now boots the pinned Rocky 10 UBI init userspace through systemd as
  virtual PID 1, reaches managed `console-getty.service`, `getty.target`, and
  `multi-user.target` within the 50-second readiness bound, authenticates root
  through `pam_unix`, and runs the resulting shell as a supervised instance task.
- Shared networking passed outbound DNS/HTTPS and inbound
  bind/listen/accept/address-conflict checks. Rocky's enabled upstream
  `baseos`, `appstream`, and `extras` repositories completed a real
  `dnf -y update` of 58 packages in the disposable root; DNF printed
  `Complete!`, the immutable OCI cache remained unchanged, and systemd powered
  off through the bounded virtual lifecycle path.
- `make test-systemd` passed three restricted-network cached-image boots plus
  a deterministic failed-DNF transaction, configured startup failure, and
  controlled teardown. The failed transaction left the sorted `rpm -qa`
  package set readable, nonempty, and unchanged and did not change the cached
  OCI rootfs SHA-256. `make bench`
  recorded 14,772 ms boot-to-login, 19,668 KiB peak traced-tree RSS,
  14,716 KiB login-ready RSS, 37 peak managed tasks, and 4 idle virtual
  `/proc` entries under Rocky 10.2 x86-64 QEMU TCG with two vCPUs.
- Fresh `make check`, aggregate `make test`, `make test-native-vfs`, and
  `make test-oci-image` passed. The aggregate includes 46 component tests and
  all mandatory earlier-gate QEMU regressions. The native-VFS regression was
  updated to require the virtual PID field introduced by multi-task audit.
- `make demo-gate-9` passed all eight automated checks after its live DNF
  watchdog was sized to observed TCG variance. Human review then clarified
  that this Expect-driven transcript is regression evidence, not the required
  human demonstration.
- The corrected `make demo-gate-9` now boots and logs the operator into the
  QEMU Rocky host with Ginux stopped, prints the manual launch command, attaches
  the terminal to the instance login and shell, and returns to the host after
  controlled shutdown. `make test-systemd-interactive` passed that exact path,
  including arbitrary command execution, retained audit evidence, host exit,
  and QEMU cleanup. Fresh `make check` and `make test-systemd` also passed after
  the staging refactor and console/audit separation. At that checkpoint, Gate 9
  remained partial pending human judgment.
- Human review on 2026-11-05 further clarified that the host must be a general
  Ginux development environment, not a Gate-specific staged demonstration.
  It must install Ginux in `PATH`, begin with no instance, and expose minimum
  persistent `ginux instance create/list/start/status/stop` commands including
  attached and detached starts. This created corrective Gate 9.5.
- The general-host workflow is now implemented. Fresh
  `make test-systemd-interactive` began with no instances, created and inspected
  `rocky` through the installed `ginux` command, rejected duplicate creation,
  reached an authenticated attached console, returned after controlled
  shutdown, exercised detached start/status/stop, detected stale PID identity,
  and cleaned up the VM. Fresh `make check`, `make test-systemd`, and aggregate
  `make test` also passed. The human accepted Gates 9 and 9.5 on 2026-11-06
  after the final interactive workflow and evidence review.

## Open decisions and limitations

- Human direction selected the security-focused handcrafted x86-64 interpreter
  for Gate 3 and accepted its evidence. On 2026-11-03 the human selected native
  host ELF execution under continuous ptrace supervision for Gate 5
  compatibility. The host handles instructions, ELF loading, memory, faults,
  signals, and scheduling; Ginux must stop every syscall before effect and fail
  closed on lost tracing. Approved passthrough calls require ptrace entry/exit
  stops because `PTRACE_SYSEMU` always skips the syscall implementation.
- Rocky host image is pinned to official Rocky 10.2 GenericCloud build
  10.2-20260525.0. Gate 2 selected a pinned, network-disabled
  Docker/LinuxKit helper that mounts a temporary raw conversion read-only;
  performance budgets remain a later-gate decision.
- Gate 2 preparation requires Docker Desktop and a pinned privileged helper
  container. It is network-disabled and mounts only a temporary raw conversion
  read-only, but remains trusted build tooling rather than a runtime boundary.
- Product, source, and cache identifiers use Ginux. Gate 8.5 joined the
  accepted userspace-kernel lineage into this repository as `d337713b`; the
  final standalone kernel remains preserved on `bootable-kernel` and
  `bootable-kernel-final`.
- The selected Rocky OCI image has one layer; deterministic synthetic fixtures
  now cover general multi-layer replacement, whiteout, and opaque-directory
  semantics.
- The native Bash proof uses a read-only unprivileged chroot and limits kernel
  descriptors to immutable image content and the bootstrap console. Gate 6
  adds Ginux-owned device objects rather than expanding native passthrough.
- Gate 6 uses a Ginux-owned isolated packet queue as its mediated
  network backend. It does not provide host or external network connectivity;
  later connectivity backends must preserve the same virtual interface and
  mediation boundary.
- Unicorn Engine 2 is not part of the Gate 5 path. It is preserved as deferred
  Gate 14, with reconsideration required after the native compatibility path is
  demonstrated and before production-boundary claims are made.

## Exact next action

For Gate 25, implement the approved public and private command model without a
public `diff` command. Gate 11 remains partial and Gate 14 remains explicitly
deferred.

## Context recovery

- Active skill: `hdae-development`
- Skill path: `.agents/skills/hdae-development/SKILL.md`
- Reload after context shift: required
- Active gate: Gate 25 command design is approved and implementation is active.
  Gate 11 remains partial and Gate 14 remains deferred.
- Active work: Gate 12 is technically passed and delivered in pull request 251;
  Gate 12.1 is technically passed and delivered in pull request 252; Gate
  12.25 is technically passed and delivered in pull request 253. Gate 12.5 is
  technically passed and delivered in pull request 254 from
  `hdae/contract-maintenance-lock`. Gate 13 was accepted and closed on
  2026-11-10 on `hdae/boundary-hardening`, rebased onto current `main`: its
  boundary, recovery, authority, regression, documentation, demonstration, and
  matched accepted-Ginux performance evidence pass. Gate 11 remains
  independently partial on its native-parity target.
- Gate 13 delivery: pull request
  https://github.com/gmkurtzer/ginux/pull/255 is open with passing required CI
  from `hdae/boundary-hardening` against `main`.
- Gate 12.25 delivery: pull request
  https://github.com/gmkurtzer/ginux/pull/253 is open from
  `hdae/top-level-instance-cli` against the Gate 12.1 branch.
- Delivery: pull request https://github.com/gmkurtzer/ginux/pull/249 is open
  against `main`; it must not be merged or closed without explicit human
  authorization.
- Gate 12 delivery: pull request https://github.com/gmkurtzer/ginux/pull/251 is
  open from `hdae/contract-network-ux` against the Gate 11 branch.
- Gate 12.1 delivery: pull request https://github.com/gmkurtzer/ginux/pull/252
  is open from `hdae/virtual-kernel-filesystems` against the Gate 12 branch.
- Last evidence: Gate 19 passed `make test-generic-oci`,
  `make demo-gate-19`, `make test-gate19-native-examples`, `make check`,
  `make test-config`, `make test-gate975-hooks`, `make test-security`,
  `make test-systemd`, and aggregate `make test`. Gate 12.1 also passed `make
  check`, `make test-kernel-views`,
  `make test-process`, the complete `make test-systemd` matrix, aggregate
  `make test`, and `make demo-gate-12.1`. The demo proved on-demand proc/sys
  access, live virtual-PID lifecycle, virtual metadata, no host leakage, empty
  backing mountpoints, multi-user systemd boot, and controlled shutdown.
- Gate 12.25 evidence: `make check`, `make demo-gate-12.25`, `make
  test-instance`, `make test-config`, `make test-systemd`, `make
  test-systemd-interactive`, and aggregate `make test` passed.
- Gate 12.5 evidence: `make check`, `make test-gate125-lock`, the complete clean
  Rocky `make test-systemd-interactive` workflow, `make test-config`, `make
  test-instance`, `make test-systemd`, aggregate `make test`, and `make
  demo-gate-12.5` passed. Rocky proved locked denial, unlocked maintenance
  success, and denial after relocking while retaining mandatory host-boundary
  backstops.
- Gate 13 evidence: `make check`, five-second-per-target `make fuzz-security`,
  `make test-security`, `make test-recovery`, `make test-contract`, aggregate
  `make test`, and `make demo-gate-13` pass. Fresh five-second/five-repetition
  current-main and Gate 13 matrices are environment-compatible and pass every
  accepted-Ginux regression row within 4%; the worst ratio is 0.9718x and all
  thread-yield ratios are 0.9760x or better. Gate 11's absolute native-parity
  limitation remains documented and unchanged. Post-review evidence also
  passes `make test-host-lifecycle`, `make test-config`, and the complete
  `make test-systemd-interactive` clean/cache/audit/persistence workflow;
  ordinary cleanup left the live interactive host and its overlay intact.
  Attached-console follow-up evidence also passes a focused Ctrl-C/Ctrl-Z/
  Ctrl-]/reattach/poweroff session. Commit `5da5e868` additionally orders
  `SIGCHLD` delivery after virtual child-exit recording; a focused session and
  the complete `make test-systemd-interactive` workflow now prove that
  `sleep 2 &` reports `Done` on the next prompt. Post-fix `make check` and
  aggregate `make test` also pass.
- Delivery audit on 2026-11-09 verified pull requests 249, 251, 252, 253, 254,
  and 255 are open, non-draft, cleanly mergeable, correctly stacked, and have
  passing `static-and-unit` and `launch-integration` checks. No pull request was
  merged or closed. Gate 14 is the only later ledger entry and remains under
  `gates-deferred/`.
- Completed paranoid work: the benchmark harness records dispatch mode and
  rejects cross-mode baseline comparison. The retained single-broker path has
  constant-time and precomputed contract decisions, bounded audit, and no
  redundant host yield. A long full matrix, audit-disabled matrix, perf
  profile, CPU-separation test, and reverted multi-broker diagnostic exhaust
  the credible invariant-preserving paths.
- Approved network direction: Gate 9 defaults to full host-shared inbound and
  outbound networking with normal host routing and bind conflicts, under Ginux
  mediation and audit. Gate 9.75 adds comprehensive instance configuration,
  network selection, port controls, and translation.
- Gate 9.75 closure: pull request 229 merged as `8a475e8b`; human accepted and
  closed the gate on 2026-11-06.
- Required human decision: choose whether Gate 11 should require native
  x86-64/KVM confirmation, adopt mode- or workload-specific budgets, or keep
  the current 4% threshold and leave Gate 11 partial. Any kernel-side
  enforcement component or weakening of paranoid all-syscall userspace
  mediation requires a separate architecture decision.
