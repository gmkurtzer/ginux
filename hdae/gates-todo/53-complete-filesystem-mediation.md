# Complete filesystem mediation

## Source

[Issue #290](https://github.com/gmkurtzer/ginux/issues/290) and its complete
[FS-001 through FS-019 negative specification](https://gist.github.com/scadastrangelove/3fa53e5e3062f8b794c8f800db31bb90).
Gate 31 supplied the accepted foundation; this gate completes the operation
families and closes the umbrella requirement.

## Goal

Make every filesystem-capable x86-64 entry follow one explicit, testable Ginux
route so supported Linux filesystem behavior works inside the instance while no
path, descriptor, alias, race, indirect interface, failure, or lifecycle change
can read, mutate, disclose, or reconnect host or peer filesystem authority.

## Approved scope

- Satisfy every normative requirement FS-001 through FS-019 from issue #290.
- Reuse and extend Gate 31's immutable call context, complete operation
  inventory, typed capabilities, generation model, and independent host-effect
  oracle; do not create a parallel authority model.
- Enable operation families in this order, widening the claimed surface only
  after the preceding family passes its positive, negative, race, and mutation
  evidence: observation and directory state; safe open; descriptor I/O;
  single-path mutation; two-path mutation; then special interfaces and
  lifecycle recovery.
- Cover configured launch, native supervision, mode 3/systemd execution, every
  supported contract and dispatch mode, loader access, and copy-in/copy-out.
- Maintain an independently pinned x86-64 filesystem-capable syscall set and a
  route record for every entry, including exact supported flags, rights,
  executor, and unsupported errno.
- Implement FS-T01 through FS-T09 with independent object/effect oracles,
  deterministic race barriers, fault injection, property checks, and required
  security mutations.
- Preserve normal Linux guest-root semantics selected in Gate 31: absolute
  paths and absolute symlinks start at the instance root, root-level `..`
  remains at the root, scoped ordinary links work, and magic links require
  explicit authority.
- Update operator/security documentation and the inventory to state supported
  operations, explicit denials, limits, compatibility, and residual risks.
- Perform a final source-level security review and full clean verification at
  the exact pull-request head.

## Non-goals

- Do not grant arbitrary host volumes, host descriptors, mount namespaces,
  device nodes, opaque handles, or asynchronous filesystem submission merely
  to increase compatibility. Explicit, tested denial satisfies issue #290 for
  interfaces outside the accepted instance contract.
- Do not weaken contract checks, generation checks, immutable-image policy,
  virtual `/proc`/`sys`/`dev`, or existing network and process boundaries.
- Do not claim complete Linux VFS compatibility, production certification, or
  protection from a compromised host kernel.
- Do not change the default execution or dispatch profile.

## Completed work

- [x] Human approved one comprehensive gate covering all of issue #290 on
      2026-11-25.
- [x] Re-read the complete issue specification and mapped FS-001 through
      FS-019 and FS-T01 through FS-T09 into this gate.
- [x] Confirmed Gate 31 already supplies FS-001 through FS-003 foundations,
      typed root/cwd/descriptor authority, inventory equality, route
      interlocks, and oracle-sensitivity checks.
- [x] Confirmed current `main` at `e545062b` still leaves
      `fs_hooks::claims()` false for every syscall.
- [x] Reconciled the inventory with one accountable executor per row and
      ordered every row into observation, safe-open, descriptor-I/O,
      single-path, two-path, or special-interface families.
- [x] Selected the existing kernel-held chroot/cwd/open-file objects as the
      race-resistant byte-preserving resolver and executor: Ginux authorizes
      the stopped syscall context and never reopens its checked path in the
      supervisor.
- [x] Enabled the observation and directory-state family through the complete
      call-context, contract, capability, and confinement conjunction.
- [x] Enabled safe open/openat through the same kernel-held root/cwd route;
      unsupported `creat` and `openat2` spellings retain explicit `ENOSYS`
      through their accountable core decision instead of native fallback.
- [x] Made descriptor and indirect-I/O routes require a current
      instance/lifetime/table generation plus API-equivalent read, write,
      create, or metadata rights; writable shared mappings and two-descriptor
      transfers require both read and write authority.

## Remaining work

- [ ] Complete operation-by-operation support/flag/errno documentation and
      final FS-001 through FS-019 traceability.
- [ ] Extend deterministic resolver/race/mutation evidence across every later
      path family; the shared confined-kernel route is implemented.
- [ ] Enable and verify single-path mutation with immutable-image and
      permission enforcement.
- [ ] Enable and verify two-path mutation with independent authorization of
      every source, destination, parent, and replacement object.
- [ ] Complete procfs/magic-link, device/mount/alias, process lifecycle,
      teardown, cancellation, failure-commit, and resource-bound coverage.
- [ ] Run all nine mandatory suites, required security mutations, full project
      regressions, and exact-head CI.
- [ ] Deliver the verified implementation in one dedicated pull request that
      references issue #290, then stop for human security review.

## Acceptance criteria

- **FS-001:** every handler consumes one immutable ABI-complete context with
  all six arguments, task/instance identity, root/cwd/descriptor authority,
  generations, and effective contract state.
- **FS-002:** every filesystem-capable entry and accepted variant has exactly
  one owner, executor, and unsupported result; unknown ABI, syscall, or flags
  fail before host effect.
- **FS-003:** contract permission, route confinement, object ownership, rights,
  and current generations are conjunctive on every successful route.
- **FS-004:** absolute, cwd-relative, and dirfd-relative resolution retain
  distinct meanings and use stable instance capabilities, never re-opened
  provisioning strings.
- **FS-005:** byte-preserving component resolution handles links before later
  parents, validates intermediate directories and final-component semantics,
  and never treats lexical prefixes as containment.
- **FS-006:** authorization and execution use the same protected object or
  race-resistant operation; no checked path is reopened by name.
- **FS-007:** every operand and parent of rename, link, copy-like, and related
  mutations is independently authorized through deterministic race windows.
- **FS-008:** operations that act on a link entry never accidentally follow its
  target, while follow operations use the authorized resolver.
- **FS-009:** descriptor authority is object- and rights-based; duplication,
  sharing, close/reuse, and unlinked-file lifetime cannot revive stale rights.
- **FS-010:** startup, exec, clone, standard streams, SCM_RIGHTS, pidfd,
  procfs-fd reopening, and other imports cannot introduce unapproved host
  handles.
- **FS-011:** cwd, root, fork/clone/exec, and intentional same-instance
  `CLONE_FS` sharing preserve initialized, generation-valid authority.
- **FS-012:** virtual proc/sys views disclose no host path or capability and
  procfs magic links cannot become ordinary host links.
- **FS-013:** devices, mounts, aliases, namespace operations, and provisioning
  require typed ownership or explicit denial; names never establish identity.
- **FS-014:** empty-path forms, opaque handles, metadata calls, mappings,
  splice/sendfile/copy operations, and async mechanisms preserve equivalent
  ownership and write restrictions or fail explicitly.
- **FS-015:** bounded supervisor-owned snapshots are copied once and completely
  validated; short/partial copies, mutable tails, invalid flags, and unsafe
  copy-out fail before unauthorized effect or disclosure.
- **FS-016:** resolved-object permissions, credentials, contract rights, and
  immutable-image state apply consistently across aliases and API families.
- **FS-017:** injected errors identify the correct stage, pre-authorization
  failures leave no mutation, committed partial results follow documented
  semantics, and temporary authority is released.
- **FS-018:** bytes, components, link depth, retries, descriptors, work, memory,
  and audit data have explicit bounds whose exhaustion cannot weaken routing or
  another instance.
- **FS-019:** cancellation, task death, stop/recreate, and supervisor loss
  cannot apply stale work or authority to a replacement root or peer.
- Supported loader, cwd, link, descriptor, writable-overlay, systemd, package,
  recovery, and multi-instance behavior remains green; blanket denial is not a
  passing implementation.
- Required fault and security mutations are reached and killed, including
  lexical-prefix containment, missing magic-link protection, checked-path
  reopening, one-ended mutation checks, inherited-directory handles, stale fd
  generations, ignored tail arguments, hook-before-contract execution, and
  resolver-failure fallback.
- Final review reports no unresolved in-scope high- or critical-impact finding,
  and every criterion has fresh exact-head evidence.

## Verification plan

- `make check` -> format, strict lint, generated inventory, independent ABI-set
  equality, route ownership, and static security checks pass.
- `make test-unit` -> all component, authority, resolver, snapshot, rights,
  error, lifecycle, and documentation tests pass.
- `make test-native-vfs` -> FS-T01 namespace walking, supported loader/open/I/O,
  and exact-error compatibility pass on the supported x86-64 environment.
- `make test-fs-races` -> FS-T02 deterministic exchanges at every resolution
  and commit barrier never reach a host/peer object.
- `make test-fs-mutations` -> FS-T03 source/destination/parent combinations and
  replacement/exchange flags preserve two-ended authority.
- `make test-fs-descriptors` -> FS-T04 inheritance, dup, close/reuse, import,
  empty-path, cwd, fork/exec, and unlinked lifetime pass.
- `make test-fs-special` -> FS-T05 proc magic links, devices, mounts, aliases,
  opaque handles, and socket endpoints are virtualized or explicitly denied.
- `make test-fs-indirect-io` -> FS-T06 mappings, transfer calls, and admitted
  async variants enforce rights equivalent to ordinary I/O.
- `make test-fs-faults` -> FS-T07 injected acquisition, copy, resolution,
  commit, return, and cleanup failures preserve errors and leak no authority.
- `sudo make test-native-supervision test-systemd` -> FS-T08 covers every
  advertised confined native route, contract, and dispatch mode.
- `make test-recovery test-multi-instance` -> FS-T09 cancellation,
  supervisor loss, stop/recreate, stale completion, peer isolation, and normal
  persistence pass.
- `make test-security` -> independent tripwire and transient-effect oracles,
  property checks, and every required security mutation pass.
- `make test-contract make test-rpm` -> contract ordering, packaged loader and
  filesystem behavior, upgrade/reinstall persistence, and offline operation do
  not regress.
- `make test` -> the complete aggregate suite passes from a clean state.
- Fresh pull-request CI at the exact final head -> all required jobs pass.
- Final forward, reverse, coverage, and orphan audits -> every FS-001 through
  FS-019 requirement maps to implementation, FS-T01 through FS-T09 evidence,
  documentation, and one accountable inventory route.

## Evidence

- Current baseline `e545062b`: `fs_hooks::claims()` returns false for every
  syscall; Gate 31's context, inventory, capabilities, and oracles are present.
- Main workflow run 45708975262 passed changes, static/unit/release-overflow,
  build, launch, native supervision, process-native, setup, and tracee-loss
  checks before this gate began.
- Baseline `make check && make test-unit` passed before implementation.
- Observation-family implementation: `make check` passed; a fresh isolated
  Rocky 10 x86-64 QEMU run of `test-native-vfs` passed loader/open/I/O,
  mutation and network denials, bounded supervisor authority, supervisor-loss
  cleanup, unexpected-stop cleanup, and immutable-image verification. The run
  used the existing checksum-pinned OCI and host-image caches because this
  dedicated worktree intentionally starts without duplicate 570+ MiB caches.
- Safe-open checkpoint: `make check` and a second fresh isolated Rocky 10
  x86-64 `test-native-vfs` run passed. The positive loader/open path, explicit
  mutating-open denial, and immutable-root digest check all remained green.
- Descriptor/indirect-I/O checkpoint: `make check` and `make test-unit` passed,
  including current/stale generation and writable-mapping rights tests. A
  third isolated Rocky 10 x86-64 `test-native-vfs` run passed real loader and
  file I/O plus cleanup and immutable-image checks with the descriptor guard
  active.

## Decisions and limitations

- The human explicitly required this gate to cover all of issue #290 rather
  than only the first observation family.
- The accepted Gate 31 compatibility decisions remain authoritative: Linux
  guest-root parent behavior; scoped ordinary symlinks; explicit magic-link
  authority; default denial of mount crossings and arbitrary external
  descriptors; no live-root movement without revocation; explicit limits; and
  no weak resolver fallback.
- All operation families live in one gate and one pull request because they
  jointly establish the one filesystem authority boundary requested by issue
  #290. They remain ordered internal milestones with separately reviewable,
  signed commits and tests; no family widens the surface before its evidence
  passes.
- Explicit denial is the supported outcome for capabilities outside the
  accepted contract. Covering FS-001 through FS-019 does not imply implementing
  host mounts, arbitrary volumes, opaque handles, or `io_uring`.
- This is a substantial security checkpoint. Technical passage does not
  authorize merge, issue closure, or movement to `gates-done`.

## Delivery strategy

- Working branch: `hdae/filesystem-mediation-complete`
- Intended base: `main`
- Pull request: intended; must reference issue #290 and contain the complete
  corrective implementation, tests, documentation, and gate evidence before
  opening
- Planned commits: gate and traceability baseline; resolver and immutable
  snapshots; observation/directory family; safe open and descriptor authority;
  descriptor/indirect I/O; single-path mutation; two-path mutation; special
  interfaces and lifecycle; test architecture and security mutations;
  documentation, final review, and evidence
- Actual commits: `284825c5` observation-family route; safe-open checkpoint
  `931c701a`; descriptor/indirect-I/O checkpoint pending commit

## Review policy and outcome

Human security checkpoint. The focused agent may implement, repair, verify,
make signed commits, push the dedicated branch, and open or update the one
referencing pull request. Stop after complete exact-head evidence for explicit
human review. Do not merge the pull request, close issue #290, or move this gate
to `gates-done` without explicit human acceptance.
