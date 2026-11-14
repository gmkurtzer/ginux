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

## Remaining work

- [ ] Add the complete immutable filesystem call context and route adapters.
- [ ] Add and validate the operation-ownership inventory.
- [ ] Add typed root, cwd, dirfd, descriptor, and lifetime authority.
- [ ] Add the independent host-effect, race, and mutation test architecture.
- [ ] Prove both confined native routes preserve contract and confinement
  conjunction without enabling a filesystem syscall.
- [ ] Perform the explicit security-focused review and resolve findings.
- [ ] Run the complete final test matrix and record fresh evidence.
- [ ] Deliver the dedicated pull request and stop for human review.

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
  ownership checks pass.
- `make test-unit` -> complete-context, capability-generation, inventory, and
  independent object-model tests pass.
- `sudo make test-native-supervision` -> both configured native routes preserve
  contract-first handling and consume supervisor-owned context.
- `make test-native-vfs` -> existing loader, VFS, root, descriptor, and denial
  behavior remains unchanged.
- `make test-contract` -> contract denial remains authoritative before any
  filesystem hook result.
- `make test-security` -> host/peer tripwires, deterministic races, fault
  injection, and security mutations pass.
- `make test-systemd` -> supported systemd filesystem behavior does not regress.
- `make test` -> the complete aggregate suite passes at the final branch head.
- Final source review -> trace every filesystem-capable route from syscall
  entry through authorization to host effect, inspect unsafe code and lifetime
  transitions, and record findings and resolutions in this gate.

## Decisions and limitations

- The strict profile uses Linux guest-root semantics: absolute paths and
  absolute symlink targets start at the instance root, and `..` at that root
  remains there.
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

## Delivery strategy

- Working branch: `hdae/filesystem-mediation-foundation`
- Intended base: `main`
- Pull request: intended; reference issue #290 without a closing keyword
- Planned commits: gate and inventory; complete call context; capability model;
  independent oracles and mutations; cross-route integration; security review
  and final evidence
- Actual commits: pending

## Review policy and outcome

Security checkpoint. The approved gate may run autonomously through
implementation, security review, complete final testing, commit, push, and pull
request delivery. Stop after delivering the pull request for explicit human
review; technical passage does not close the gate.
