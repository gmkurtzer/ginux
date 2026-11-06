# Configurable contract and audit

## Source

Core Ginux product requirement: each domain exposes an explicit, versioned,
configurable Linux contract with logging and no unmediated host syscall effect.

## Goal

Allow an operator to select a validated contract that governs syscall, ioctl,
device, filesystem, networking, and virtual-kernel behavior for one domain;
compose that contract from understandable versioned templates and bounded
selectors; and produce a complete, useful, bounded audit trail of contract
decisions.

## Approved scope

- Define a versioned contract schema referenced by normalized domain
  configuration.
- Define one versioned operation catalog covering system calls, ioctl requests,
  networking operations, devices, filesystem operations, `/proc`, `/sys`,
  cgroups, credentials, process control, signals, time, memory, and the other
  kernel/userspace interfaces supported or deliberately denied by Ginux.
- Track each catalog entry's canonical identifier, Linux ABI identity,
  implementation status (`complete`, `partial`, `denied`, `planned`, or
  `out-of-scope`), contract action, argument or resource constraints, profiles,
  tests, and known limitations. Generate the human-readable ABI coverage
  document from this catalog so documentation and enforcement cannot silently
  drift.
- Express emulate, mediated, and deny decisions; there is no direct-continue
  decision.
- Cover syscall numbers and argument constraints, admitted ioctl families,
  device classes, filesystem operations or paths, and virtual kernel views used
  by the systemd baseline.
- Provide versioned default templates, including the tested Rocky systemd
  contract and a minimal deny-oriented base. Templates may include other
  explicitly versioned templates, but cycles, missing versions, and ambiguous
  composition fail validation.
- Allow bounded glob selectors over canonical hierarchical identifiers such as
  `syscall.clock_*`, `ioctl.tty.*`, `network.inet.tcp.*`,
  `device.tty.*`, `proc.process.*`, and `sys.kernel.*`. Filesystem and virtual
  tree policies may additionally use documented component-aware path globs;
  network resource constraints use explicit address families, CIDRs, address
  selectors, protocols, and port ranges rather than treating addresses as
  arbitrary strings.
- Expand every template and selector against a pinned catalog version during
  normalization and record the resulting explicit operation table. A selector
  that is invalid or matches nothing fails validation. A later catalog version
  must not silently grant newly introduced operations through an older glob.
- Apply deterministic layers: deny-by-default catalog behavior, one versioned
  base template, ordered versioned template additions, domain-file overrides,
  then explicit command-line overrides. Exact and patterned conflicts must
  resolve by documented layer and specificity rules; unresolved conflicts fail
  closed, and the effective contract records the winning rule and provenance.
- Validate the complete contract before starting managed code and fail closed
  on unknown security-critical syntax or unsupported requirements.
- Make the effective contract immutable for the lifetime of the initial domain.
- Emit structured, rate-aware audit events for decisions, denials, malformed
  requests, lifecycle, and effective contract identity.
- Provide at least a systemd-compatible profile and a stricter negative-test
  profile.

## Non-goals

- Arbitrary policy code, runtime contract mutation, remote policy distribution,
  or automatic policy generation.
- Unbounded regular expressions, runtime wildcard matching, unversioned
  templates, or globs that automatically acquire authority when the operation
  catalog grows.
- Logging raw secrets, arbitrary managed memory, or unbounded attacker-controlled
  payloads.
- Treating audit-only observation as enforcement.

## Completed work

- [x] Added catalog version 1 with bounded canonical operation identifiers,
  Linux ABI numbers, implementation status, action, constraints, evidence, and
  limitations across the approved kernel-facing interface families.
- [x] Added deterministic deny-by-default resolution for the versioned
  `minimal-v1` and `gate9-systemd-v1` profiles, versioned additive templates,
  bounded selectors, explicit domain allow/deny layers, provenance, and a
  derived immutable contract digest.
- [x] Connected normalized domain configuration and command-line overrides to
  transactional contract validation; unknown profiles, unmatched selectors,
  forged derived digests, and invalid overrides fail without mutating the
  accepted configuration.
- [x] Connected native syscall dispatch to the resolved explicit contract so a
  policy denial overrides implementation support while an allow cannot promote
  an unsupported syscall.
- [x] Generated `docs/abi-contract-coverage.md` from the catalog and added a
  dispatcher/catalog/document consistency check.
- [x] Added a fixed audit-event budget with logarithmic dropped-event reports;
  structured native records carry domain and effective contract identity.
- [x] Added `ginux contract-plan` effective-contract inspection plus operator
  documentation for selector precedence, typed resource constraints, profiles,
  templates, examples, provenance, digests, and audit bounds.
- [x] Connected semantic filesystem, virtual-tree, ioctl, device, networking,
  memory, signal, time, and credential families to native enforcement without
  allowing a semantic grant to promote an unsupported syscall.
- [x] Added strict and systemd profile fixtures, real Rocky runtime denial and
  host-mutation checks, malformed-policy and immutable-contract tests, bounded
  audit-flood evidence, and the Gate 10 human demonstration.

## Remaining work

- [ ] Human review and acceptance of the Gate 10 demonstration and pull request.

## Acceptance criteria

- Every managed syscall and admitted ioctl in the tested profiles has an
  explicit effective decision and contract version.
- The operation catalog covers syscalls, ioctl families, networking, devices,
  filesystems, `/proc`, `/sys`, cgroups, signals, process authority,
  credentials, time, memory, and every other kernel-facing interface reached by
  the accepted systemd workload. Each entry records implementation status and
  executable evidence or an explicit gap.
- A generated ABI coverage document lists what Ginux implements, partially
  implements, denies, plans, and excludes, with totals and links to tests. A
  consistency check fails when dispatch code, templates, catalog entries, or
  generated documentation disagree.
- Versioned templates expand reproducibly into the same explicit effective
  operation table and digest. Template cycles, unpinned references, unmatched
  selectors, invalid path globs, invalid CIDRs or port ranges, and ambiguous
  rules fail before domain startup.
- Globs can select broad syscall, ioctl, networking, device, and virtual-tree
  families, while exact domain TOML rules can deliberately grant or restrict a
  specific member. The effective-contract output identifies the template,
  selector, override, and final action responsible for every operation.
- Expanding an old contract against a newer operation catalog never grants a
  newly added operation implicitly; it remains denied until the contract is
  explicitly upgraded and revalidated.
- No policy action permits an unchecked managed syscall to continue on the
  host; native passthrough must first validate the operation and arguments.
- Unknown operations and unknown security-critical policy fields fail closed.
- The strict profile denies its selected operations while the systemd profile
  retains the accepted systemd baseline.
- Contract denial prevents the corresponding host effect and emits the expected
  bounded audit record.
- Audit records identify domain, effective contract, operation, decision, and
  result while redacting or bounding configured sensitive data.
- Audit flooding cannot grow memory or disk use without the documented bound
  and exposes dropped-event accounting.
- Effective configuration and contract evidence can reproduce which policy was
  active for a test result.
- `make demo-gate-10` lets a human compare permissive and strict contracts,
  observe a mediated operation and denial, and inspect bounded audit records.

## Verification plan

- `make test-contract` -> schema, normalization, unknown-field, profile,
  template composition, glob expansion, exact override, conflict, catalog
  version, emulate, mediate, deny, bypass-negative, immutable-policy, and audit
  tests pass.
- Contract inventory check -> every dispatcher operation and supported virtual
  interface maps to one catalog entry; every catalog entry maps to enforcement
  and evidence or an explicit gap; generated documentation is current.
- Selector fixtures -> syscall, ioctl, networking, device, path, `/proc`, and
  `/sys` family selectors expand to expected explicit operations; zero-match,
  malformed, ambiguous, and catalog-growth fixtures fail closed.
- Template fixtures -> minimal and Rocky systemd templates compose
  deterministically; a domain TOML restricts a broad template grant and
  deliberately grants one exact operation without changing unrelated rules.
- Systemd profile: `make test-systemd` -> remains passing.
- Strict profile negative fixtures -> operations fail with expected results,
  host-state snapshots remain unchanged, and matching audit events exist.
- Audit flood fixture -> resource bounds and dropped-event accounting match the
  documented policy.
- `make test` -> all mandatory regression suites exit zero.
- `make demo-gate-10` -> performs the documented profile comparison, mediation,
  denial, audit, and cleanup demonstration and exits zero.

## Evidence

- `make test-contract` -> 9 contract tests and 18 domain tests passed; generated
  documentation matched the 182-operation catalog.
- `cargo build -p ginux-init --target x86_64-unknown-linux-gnu --release` ->
  native runtime contract enforcement and bounded audit integration built.
- `make check` -> formatting and strict lint passed across every workspace
  component and x86-64 fixture.
- `ROCKY_SSH_PORT=22231 make test-contract-runtime` -> the configured domain
  denial blocked `socket`, produced the structured `domain-deny` audit record,
  and left no host mutation.
- `ROCKY_SSH_PORT=22232 make test-systemd` -> cold, warm, repeat, offline, DNF
  failure, configured-startup failure, audit bound, and shutdown checks passed.
- `ROCKY_SSH_PORT=22233 make test-gate975-network` and
  `ROCKY_SSH_PORT=22234 make test-gate975-hooks` -> accepted networking,
  translation, policy, and mediated hook behavior remained passing.
- `ROCKY_SSH_PORT=22235 make test` -> all 83 component tests and every mandatory
  QEMU interception, runtime, configuration, domain, image, device, process,
  and virtual-kernel-view regression passed.
- `ROCKY_SSH_PORT=22237 make demo-gate-10` -> passed after the demonstration
  exposed and corrected a console-interleaving assumption in its marker check.
  It displayed distinct strict and systemd plans, the enforced socket denial,
  absent host mutation, an actual mediated systemd operation, exactly 8,192
  detailed audit records, logarithmic dropped-event accounting through 65,536
  drops, and controlled poweroff.

## Decisions and limitations

- Schema syntax is chosen in Gate 4 for domain configuration and extended here
  only as needed for contract semantics.
- Templates and globs are operator conveniences, not runtime enforcement
  primitives. Enforcement consumes only the normalized explicit table tied to
  a catalog version and digest.
- Semantic capability names remain the preferred operator interface. Low-level
  syscall and ioctl selectors exist for precision, debugging, and workloads
  that cannot be described solely through a higher-level capability bundle.
- Runtime policy changes are deferred because they require authority,
  synchronization, and audit rules not approved here.
- The catalog documents bounded Linux-compatible behavior rather than claiming
  complete Linux ABI coverage; `partial`, `planned`, and `out-of-scope` entries
  remain explicit gaps.

## Delivery strategy

- Working branch: `hdae/configurable-contract`
- Intended base: `main`
- Pull request: intended; dedicated branch push and PR creation authorized on
  2026-11-03; delivered as https://github.com/gmkurtzer/ginux/pull/231;
  never merge or close without explicit authorization
- Planned commits: ABI catalog and generated coverage document; template,
  selector, and override schema; dispatcher and backend integration; structured
  audit and bounds; profiles and security regressions
- Actual commits: `410d9e8f` (versioned operation catalog), `7f0eeab6`
  (normalized domain contracts), `25221a0b` (profiles and bounded audit), and
  `fb559dc6` (semantic family enforcement), and `dfeeaaa5` (final verification
  evidence)

## Review policy and outcome

Human checkpoint because this gate defines the operator-visible security
contract and the evidence used for later kernel-exposure claims. Present
`make demo-gate-10`, fresh evidence, and the pull request before acceptance.
The implementation is technically passed and remains open pending that review.
Pull request 231 is open and unmerged.
