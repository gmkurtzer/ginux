# Filesystem mediation foundation

## Source

[Issue #290](https://github.com/gmkurtzer/ginux/issues/290) and its complete
[negative specification](https://gist.github.com/scadastrangelove/3fa53e5e3062f8b794c8f800db31bb90),
reviewed against `main` at `285d025b`.

## Goal

Establish the complete syscall context, explicit operation ownership, typed
root/cwd/descriptor authority, and independent host-effect oracle required
before the native filesystem lane claims any syscall.

## Approved scope

- Replace the filesystem lane's three-argument interface with a bounded,
  supervisor-owned call context containing the x86-64 ABI, all six syscall
  arguments, task and instance identity, instance lifetime/root generation,
  stable root and cwd authority, authoritative descriptor-table identity, and
  effective contract decision.
- Maintain a complete filesystem-operation inventory recording each supported
  ABI entry's semantic operation, arguments and flags, owner, required
  authority, host-effect executor, and explicit unsupported result.
- Establish typed root, cwd, dirfd, and descriptor capability foundations;
  numeric descriptors and path strings alone do not grant authority.
- Interlock configured launch and configured-native mode 3 so neither route can
  bypass the same filesystem security model.
- Build an independent host-effect oracle with instance, peer-instance, and
  host-only tripwire trees, object-identity observations, mutation detection,
  and deterministic race barriers.
- Add static and mutation checks that reject incomplete context, unowned
  operations, hook-before-contract authorization, checked-path reopening, and
  ignored fourth-through-sixth arguments.
- Review the complete change as a security boundary, remediate every in-scope
  finding, and document residual limitations.
- Run every focused, security, compatibility, and aggregate regression test at
  the end of the gate.

## Non-goals

- Do not make `fs_hooks::claims()` claim a syscall in this gate.
- Do not enable new open, stat, descriptor-I/O, mutation, mount, or external
  volume behavior.
- Do not claim broad Linux VFS compatibility.
- Do not admit arbitrary imported host descriptors or host filesystem
  capabilities.
- Do not close issue #290; it remains the umbrella source until later
  operation-family gates satisfy FS-001 through FS-019.

## Completed work

- [x] Reviewed issue #290 and its complete negative specification against
  current `main`.
- [x] Confirmed `fs_hooks` still declines every call and receives only three
  syscall arguments.
- [x] Confirmed configured launch applies its contract before `decide_native`,
  while configured-native mode 3 bypasses `decide_native` and has separate
  filesystem policy paths.
- [x] Confirmed Gate 30's shared descriptor-table authority is the required
  descriptor foundation rather than numeric-fd authority.
- [x] Added a complete immutable x86-64 call context carrying all six syscall
  arguments, task and instance identity, lifetime and root generation,
  root/cwd/descriptor-table authority, and the effective contract decision.
- [x] Added the generated filesystem operation-ownership inventory and
  fail-closed lookup for unknown ABIs, syscalls, owners, and denials.
- [x] Added typed, object-identified capabilities and monotonic descriptor-table
  generations; generation exhaustion terminates the instance rather than
  reusing authority.
- [x] Added independent host/peer tripwires, deterministic path-replacement
  races, lexical-prefix rejection, and stale-generation mutations.
- [x] Interlocked configured launch and configured-native mode 3 without making
  the filesystem lane claim any syscall or adding a host effect.
- [x] Completed an explicit security review and remediated every in-scope
  implementation finding.
- [x] Ran the focused, security, recovery, aggregate, and systemd suites; all
  Gate 31 and aggregate coverage passed.
- [x] Reproduced and diagnosed the pre-existing systemd startup blocker; its
  attempted networking repair remains unaccepted and is listed below for
  separation or fail-closed correction.
- [x] Rebuilt the exact committed RPM and exercised its metadata, clean DNF
  install, verification, packaged Alpine lifecycle, packaged Rocky systemd
  boot, offline cache reuse, reinstall, erase/reinstall, and state retention.

## Remaining work

- [x] Make a claimed filesystem operation fail closed when contract,
  confinement, ownership, context, or capability validation fails; it must not
  fall through to a legacy executor.
- [x] Validate the operation inventory by exact set equality against an
  independently maintained, pinned x86-64 filesystem-capable syscall set.
- [x] Make the host-effect oracle prove its own sensitivity by deliberately
  injecting and detecting protected read, write, metadata, handle, and
  transient effects.
- [x] Remove the unrelated non-IP socket-family passthrough from this gate.
  Required netlink behavior belongs to an explicit bounded owner with
  validated or emulated semantics; unsupported socket families and message
  shapes fail before host effect.
- [x] Re-run the complete verification matrix at the repaired exact pull-request
  head, update the evidence, and stop for explicit human review.

## Acceptance criteria

- The filesystem call context exposes the x86-64 ABI, all six arguments, task
  and instance identity, lifetime/root generation, root/cwd authority,
  descriptor-table identity, and effective contract decision without reading
  mutable tracee values twice.
- Every filesystem-capable syscall in the pinned catalog has exactly one
  documented owner or explicit denial and one deterministic unsupported
  result; an unknown syscall, ABI, or flag cannot fall through to host effect.
- Contract permission, confinement, and capability rights are conjunctive on
  every future hook path; hook ownership alone never grants authorization.
- Root, cwd, dirfd, and descriptor authority is tied to instance object identity
  and generation rather than a pathname or numeric descriptor.
- Supported operations preserve normal Linux filesystem semantics inside the
  instance boundary. Unsupported capabilities fail explicitly, and no path,
  descriptor, procfs reference, mount crossing, race, or lifecycle transition
  may escape or reconnect outside that boundary.
- Replacing the configured host pathname cannot redirect an acquired root, and
  stale lifetime or descriptor generations cannot authorize a new object.
- Deterministic tests observe no read, write, metadata, handle, or transient
  effect on peer and host tripwires.
- Required security mutations are reached and killed by the tests: ignored
  tail arguments, hook-before-contract authorization, lexical-prefix
  containment, reopened checked paths, and stale capability generation.
- `fs_hooks::claims()` remains false for every syscall and no currently denied
  operation becomes allowed.
- The security review reports no unresolved in-scope high- or critical-impact
  finding and records every residual limitation.
- The final focused, security, compatibility, and aggregate test matrix passes
  at the exact pull-request head.

## Verification plan

- `make check` -> formatting, strict lint, generated inventory, and static
  ownership checks pass, including exact equality with the independent pinned
  filesystem-capable syscall set.
- `make test-unit` -> complete-context, capability-generation, inventory, and
  independent object-model tests pass; a claimed operation with missing or
  stale authority selects an explicit denial rather than a legacy executor.
- `sudo make test-native-supervision` -> both configured native routes preserve
  contract-first handling and consume supervisor-owned context.
- `make test-native-vfs` -> existing loader, VFS, root, descriptor, and denial
  behavior remains unchanged.
- `make test-contract` -> contract denial remains authoritative before any
  filesystem hook result.
- `make test-security` -> host/peer tripwires, deterministic races, fault
  injection, and security mutations pass; deliberate oracle self-mutations are
  detected, and arbitrary host netlink or abstract-Unix sends are rejected
  before host effect.
- `make test-systemd` -> supported systemd filesystem behavior does not regress,
  including the bounded positive loopback fixture without generic non-IP socket
  passthrough.
- `make test-rpm` -> the exact committed RPM installs cleanly, verifies with
  `rpm -V`, boots both packaged Alpine and Rocky systemd instances, preserves
  their state across reinstall and erase/reinstall, and stops cleanly.
- `make test` -> the complete aggregate suite passes at the final branch head.
- Final source review -> trace every filesystem-capable route from syscall
  entry through authorization to host effect, inspect unsafe code and lifetime
  transitions, and record findings and resolutions in this gate.

## Decisions and limitations

- On 2026-11-15 the human selected the normative direction for later
  operation-family gates: preserve normal Linux filesystem semantics inside
  the instance while ensuring paths and descriptors cannot escape it. Issue
  #290's requirements are adopted where they enforce that boundary; unsupported
  capabilities fail explicitly.
- The strict profile therefore uses Linux guest-root semantics: absolute paths
  and absolute symlink targets start at the instance root, and `..` at that
  root remains there.
- Scoped ordinary internal symlinks are permitted; uncontrolled procfs magic
  links are capabilities and are not followed as host links.
- Mount crossings and arbitrary external descriptors are denied unless a later
  gate introduces an explicit typed capability contract.
- Moving a live root outside its authority domain is prohibited unless an
  equivalent revocation mechanism is proven.
- Resolver and resource limits are explicit; missing required resolver or
  security support fails instance startup without a weaker fallback.
- Gate 31 establishes enforcement prerequisites only. Later gates enable
  operation families in order: observation/directory state, safe open,
  descriptor I/O, single-path mutation, two-path mutation, then special
  interfaces and lifecycle recovery.
- The implementation security review corrected contract-before-hook ordering,
  descriptor-generation overflow, overly broad root rights, missing alternate
  filesystem inventory entries, claimed-operation fallthrough, self-referential
  inventory validation, oracle sensitivity, and generic non-IP host sends.
- The supported systemd network exception is now explicit and bounded: Ginux
  snapshots and validates only immutable loopback link/address netlink
  messages, correlates their acknowledgements by sequence, rejects arbitrary
  netlink and abstract-Unix sends before host effect, confines pathname Unix
  sockets to the instance root, and reports host audit netlink as unavailable.
- Final repeated verification also found that parallel host-effect tests could
  collide when the platform clock returned the same timestamp. Fixture names
  now include a process-local atomic sequence; the oracle passed ten consecutive
  runs and the complete unit suite afterward.
- Configured launch deliberately has unavailable root/cwd/descriptor-table
  capabilities because it does not retain and generation-track those objects.
  Configured-native mode 3 has retained root and descriptor-table identity, but
  cwd remains unavailable until a later operation-family gate owns cwd object
  lifecycle. Both conditions fail closed while `fs_hooks::claims()` is false.
- Existing legacy filesystem executors remain outside this foundation's new
  compatibility claim. Their existing regressions ran, but later gates must add
  the operation-specific mutation, proc-magic-link, fault, and effect oracles
  before enabling each filesystem family.
- The systemd failure was not caused by Gate 31. The accepted repair replaces
  the rejected generic non-IP passthrough with explicit, fail-closed ownership
  for the minimum loopback and pathname-Unix behavior systemd requires.
- Ginux denies host namespace creation with `EPERM`. This preserves the
  security boundary while allowing systemd to disable an optional generator
  sandbox and continue inside the already confined instance. Truly
  unimplemented clone shapes still return `ENOSYS`.
- Installed-RPM reproduction identified a separate upgrade limitation:
  persisted normalized recipes can fail closed when catalog contents change
  without a corresponding migration. The affected instance was repaired only
  through an explicit recompile of its unchanged recipe. Silent refresh is not
  acceptable because wildcard rules may acquire new meaning; deterministic
  old-package-to-new-package migration coverage remains follow-up work.

## Evidence

- `make check` -> passed strict formatting, lint, generated inventory, and
  ownership checks, including exact equality with the independent pinned
  x86-64 filesystem-capable syscall set.
- `make test-unit` -> passed all workspace unit tests, including six policy
  mutations, claimed-operation fail-closed dispatch, five independent
  host-effect oracles, and deliberate oracle sensitivity for protected read,
  write, metadata, handle, and transient effects.
- Ten consecutive `cargo test -p ginux-fs-policy --test host_effect_oracle`
  runs -> passed after eliminating parallel temporary-directory collisions.
- `cargo clippy -p ginux-fs-policy --all-targets -- -D warnings` -> passed.
- `cargo check -p ginux-init --target x86_64-unknown-linux-gnu --release` ->
  passed.
- `make test` in an isolated Rocky QEMU host -> passed the complete aggregate
  target, including host lifecycle, runtime, image, device, process, and kernel
  view integration.
- `make test-security` in an isolated Rocky QEMU host -> passed deterministic
  fuzzing, authority reduction, contract denial, native VFS, devices, kernel
  views, teardown, and host-effect checks.
- `make test-systemd` -> passed cold and warm cached Rocky boots, repeated
  lifecycle, offline-cache reuse, failed-DNF recovery, startup failure, and
  controlled shutdown with bounded loopback netlink handling and no generic
  non-IP socket passthrough.
- Focused `sudo python3 scripts/test-native-supervision.py` in isolated Rocky
  QEMU -> passed the complete native-supervision matrix, including rejection
  of arbitrary route-netlink and abstract-Unix sends before host effect.
- `make test-security test` -> passed deterministic security coverage and the
  complete aggregate suite after the startup correction.
- Clean detached `origin/main` at `285d025b`, fresh host overlay,
  `make test-systemd` -> failed with the identical loopback `EINVAL` diagnostic,
  establishing the original failure as a pre-existing baseline defect rather
  than a Gate 31 regression.
- Installed RPM, existing `rocky` instance -> reproduced
  `contract-digest-mismatch`; explicit recompilation of the unchanged recipe
  restored its derived digest, and the corrected packaged runtime then reached
  `multi-user.target` and the Rocky login prompt.
- `GINUX_RPM_SSH_PORT=22427 make rpm test-rpm-metadata test-rpm` -> built the
  exact committed source and SRPM in Rocky, passed rpmlint with zero errors,
  installed through DNF, passed `rpm -V`, booted the packaged Alpine instance,
  booted the packaged Rocky systemd instance to `ginux login:`, reused cached
  layers without networking, preserved all instance state across reinstall and
  erase/reinstall, and stopped both test VMs cleanly.
- Final source review traced configured launch and mode 3 from immutable context
  construction through contract, confinement, inventory, capability, and hook
  guards; inspected every added unsafe/host syscall site and authority lifetime;
  confirmed no filesystem claim or new filesystem host effect was enabled and
  no unresolved in-scope high- or critical-impact finding remains.

## Delivery strategy

- Working branch: `hdae/filesystem-mediation-foundation`
- Intended base: `main`
- Pull request: https://github.com/gmkurtzer/ginux/pull/291; draft and references
  issue #290 without a closing keyword
- Planned commits: gate and inventory; complete call context; capability model;
  independent oracles and mutations; cross-route integration; security review
  and final evidence
- Actual commits: `f9aa3cf1`, `1759b054`, `b8f05a9a`, `47171760`, `718784ae`,
  `35367c74`, `2b83cdd0`, `cfd233ec`, and `c56d30c4`; final evidence checkpoint
  `c4609fbe`

## Review policy and outcome

Security checkpoint. The approved gate may run autonomously through repair,
security review, complete final testing, commit, push, and pull-request update.
Stop after delivering the repaired pull request for explicit human review.
Gate 31 is partial at exact head `c4609fbe`; passing tests do not override the
unresolved exact-head security findings, and pull-request delivery does not
close the gate.
