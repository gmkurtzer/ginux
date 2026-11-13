# Explain Ginux's architecture and differentiation

## Source

Human direction after reviewing the Hacker News discussion at
https://news.ycombinator.com/item?id=50450007.

## Goal

Make the README and supporting documentation clearly explain what Ginux is,
how it works, why it is not User-Mode Linux, where it overlaps with gVisor and
Linux isolation primitives, what is actually distinctive, and which security
or compatibility claims the current evidence does not support.

## Approved scope

- Extract the recurring technical questions and misconceptions from the
  referenced discussion.
- Explain the current syscall interception and dispatch architecture in plain
  language.
- Compare Ginux with UML, gVisor, namespaces/seccomp, and VMs or microVMs using
  authoritative project documentation.
- State Ginux's differentiators precisely: deliberately incomplete and
  versioned per-instance contracts spanning syscalls, virtual kernel state,
  filesystems, devices, networking, resources, lifecycle, and audit evidence.
- Explain why namespaces partition host-kernel objects but do not replace the
  Linux system API, and why Ginux is not a VM-equivalent security boundary.
- Explain the role of HDAE and AI without making code generation itself an
  architectural or security claim.
- Add deterministic documentation checks and update the README, architecture,
  security, and operator-facing links where useful.
- Deliver the work on a dedicated branch and pull request.

## Non-goals

- Changing runtime behavior, contracts, dispatch modes, performance, or the
  threat model.
- Claiming novelty for the userspace-kernel category or superiority over UML,
  gVisor, containers, namespaces, or VMs.
- Claiming production readiness, VM-equivalent isolation, complete Linux ABI
  compatibility, or independent security validation.
- Debating subjective reactions to AI-generated code.

## Completed work

- [x] Human approved Gate 18, its separate branch and pull request, and the HN
  discussion as the source of questions to address.
- [x] Created `hdae/explain-ginux-differences` from synchronized `main`.
- [x] Reviewed the complete HN discussion and authoritative UML and gVisor
  architecture documentation.
- [x] Rewrote the README's primary mechanism, non-goals, comparison table, and
  HDAE explanation.
- [x] Added a focused architecture comparison covering UML, gVisor,
  namespaces/seccomp, VMs, current syscall capture, trust, and differentiation.
- [x] Reconciled architecture and security-boundary documentation.
- [x] Added deterministic documentation assertions and ran all required
  documentation, static, aggregate, and CI-classification regressions.

## Remaining work

- [x] Pushed the branch and opened the Gate 18 pull request.
- [x] Obtained human review and acceptance on 2026-11-12.

## Acceptance criteria

- A reader can determine from the README, without reading source code, how
  Ginux captures syscalls and which work executes in Ginux versus host Linux.
- UML is described as a complete Linux kernel running as a host process over
  paravirtual hardware; Ginux is not described as UML or as a replacement for
  UML.
- gVisor is identified as the closest established comparison, with both the
  architectural overlap and Ginux's narrower research scope stated plainly.
- Namespaces and seccomp are distinguished from a userspace system-API layer;
  VMs and microVMs are distinguished from Ginux's current trusted-host model.
- Ginux's differentiators are concrete and testable rather than claims of
  category novelty: self-contained per-instance contracts, deliberate ABI
  reduction, integrated virtual state/resources/lifecycle/audit, and named
  compatibility evidence.
- Documentation states that the host kernel, ptrace, CPU execution services,
  and approved Ginux process remain trusted and that current evidence is not a
  production security certification.
- AI/HDAE is explained as the method used to prototype, test, and revise the
  system, not evidence that the result is secure, correct, or maintainable.
- Documentation links resolve locally and deterministic terminology/boundary
  checks pass.

## Verification plan

- Documentation assertion script -> required comparisons, trust-boundary
  statements, mechanism explanation, and local links are present.
- `make check-security-doc` -> existing security claims remain internally
  consistent.
- `make check` -> formatting, lint, schema, terminology, and documentation
  checks pass.
- `make test` -> no aggregate regression from documentation tooling changes.
- Manual hostile-reading pass -> every comparison is supported by the linked
  authoritative source and no claim exceeds current executable evidence.

## Evidence

- Hacker News review -> recurring questions cover UML, gVisor, namespaces,
  syscall capture, VM security, compatibility, ptrace overhead, project
  commitment, and what problem the system is intended to solve.
- Linux UML documentation -> UML runs a Linux kernel as a host process and
  presents paravirtual devices to its userspace.
- gVisor architecture documentation -> gVisor is an application kernel whose
  platform captures syscalls and manages address spaces; its current platforms
  include KVM, systrap, and a deprecated ptrace backend.
- `make check-architecture-doc check-security-doc check` -> passed mechanism,
  comparison, limitation, terminology, formatting, and lint assertions.
- `make test` -> passed the complete aggregate unit and QEMU integration suite.
- `make test-ci-changes` -> passed all documentation/code classification cases.
- Pull request 260 merged as `8ce4f590` on 2026-11-12.
- Follow-up pull request 262 replaced exact-prose CI assertions with document
  existence and cross-link integrity checks, then merged as `2fe3648d`.
- Manual hostile-reading pass -> mechanism language matches the checked-in
  seccomp/ptrace filter, current selective exception is limited to
  `sched_yield`, gVisor is described as closest rather than equivalent, and no
  VM-grade, complete-ABI, novelty, or production claim remains.

## Decisions and limitations

- The documentation will not claim that the userspace-kernel model is novel.
- gVisor is the closest established comparison, but Ginux is far less complete
  and mature and currently pursues an explicitly declared, workload-specific
  contract rather than broad drop-in compatibility.
- The comparison describes checked-in behavior at this gate; it is not a
  general judgment about the relative security of the projects.

## Delivery strategy

- Working branch: `hdae/explain-ginux-differences`
- Intended base: `main`
- Pull request: https://github.com/gmkurtzer/ginux/pull/260
- Planned commits: gate definition; README and comparison documentation;
  deterministic checks and evidence; pull-request record
- Actual commits: `cdf12203`, `6644b3e4`, the final evidence checkpoint commit,
  merge `8ce4f590`, and follow-up merge `2fe3648d`.

## Review policy and outcome

Accepted and closed by explicit human direction on 2026-11-12 after pull
request 260 and its documentation-check follow-up, pull request 262, merged.
