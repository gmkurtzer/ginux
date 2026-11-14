# Authoritative native shared descriptor tables

## Source

[PR #279](https://github.com/gmkurtzer/ginux/pull/279), reviewed at exact head
`5270fd2ee6de25000f5d8372cce7fa62ac305e37`.

## Goal

Make configured-native descriptor classification follow ordinary Linux
descriptor-table semantics without a window in which tasks sharing
`CLONE_FILES` can observe or permanently inherit stale Ginux state.

## Approved scope

- Preserve the contributor's fixes for shared tables, close-on-exec state,
  exec, duplication, close-range, protected console handling, 32-bit
  descriptor arguments, and thread `pidfd_open` compatibility.
- Represent each native descriptor table once in supervisor-owned state and
  have every sharing task refer to that state.
- Serialize descriptor-sensitive decisions, fork, clone, and exec against an
  in-flight mutation of the same shared table until the kernel result and
  supervisor state agree.
- Treat descriptors 0, 1, and 2 exactly like other Linux descriptors:
  classification follows the object they currently reference after
  inheritance, duplication, redirection, close, and reuse.
- Add a deterministic concurrent regression that forces a shared-table
  mutation to overlap a descriptor-sensitive operation and fork or exec.

## Non-goals

- Do not merge the broader classification change from PR #278 in this gate.
- Do not make descriptor numbers intrinsically identify console, TTY, file,
  pipe, socket, or another object.
- Do not broaden a contract or bypass existing mediation.
- Do not repair unrelated descriptor classes unless required to keep the
  shared table internally consistent.

## Completed work

- [x] Confirmed the syscall-exit propagation race in PR #279.
- [x] Received the maintainer decision for standard-descriptor semantics.

## Remaining work

- [x] Rebase the useful PR #279 changes onto current `main`.
- [x] Replace per-task copied console tracking with shared table authority and
      a mutation barrier.
- [x] Add deterministic concurrent regressions.
- [ ] Run local and Linux verification.
- [ ] Deliver a corrective pull request and reciprocal links.

## Acceptance criteria

- Tasks using `CLONE_FILES` consult one authoritative Ginux descriptor table.
- No sharer makes a descriptor-sensitive security decision or snapshots the
  table for fork or exec while a table mutation is unresolved.
- A non-sharing fork receives a consistent point-in-time copy.
- Exec atomically receives a private table with close-on-exec entries removed.
- Descriptors 0, 1, and 2 are classified solely by their referenced objects.
- A deterministic regression forces the formerly racy interleaving and proves
  that neither missed console authority nor stale reused-descriptor authority
  survives.
- Existing native-supervision, launch, static, and unit tests remain green.

## Verification plan

- `make check` -> static checks and formatting pass.
- `make test-unit` -> all unit tests pass.
- `sudo make test-native-supervision` -> shared-table, CLOEXEC, and forced
  concurrent interleaving regressions pass on Linux.
- `sudo make test-launch` -> native launch regressions pass.
- Fresh pull-request CI at the exact head -> all required jobs pass.

## Evidence

- PR #279 exact head passes its sequential tests but documents that a sibling
  may fork, exec, or make a policy decision before a completed kernel mutation
  reaches the copied Ginux state.
- The race can make a missed or stale entry durable after the sibling obtains
  a private descriptor table.
- Current implementation stores console provenance once per descriptor table,
  makes `CLONE_FILES` tasks reference that table, and defers sibling ptrace
  stops while a descriptor mutation is unresolved.
- The new Linux regression holds a conflicting host file lock so one
  descriptor mutation remains in flight, then proves a sharing sibling's
  `dup` cannot complete until the mutation barrier releases.
- `make check` passed on macOS after rebasing onto `f5ee0402`.
- `make test-unit` passed on macOS after rebasing onto `f5ee0402`.

## Decisions and limitations

- The authoritative identity is the referenced Ginux object, never the numeric
  descriptor, including standard descriptors.
- Table-level serialization is preferred over reconstructing identity from
  host paths or `/proc`.
- PR #278 remains dependent on this gate and requires its own exact-head review.

## Delivery strategy

- Working branch: `hdae/native-descriptor-table-barrier`
- Intended base: `main`
- Pull request: intended; references and supersedes the unresolved portion of
  PR #279.
- Planned commits: gate and imported contributor baseline; shared-table barrier;
  deterministic concurrency regression; evidence and delivery links.
- Actual commits: `1a5f5a36` gate definition, `9dd04b94` attributed PR #279
  baseline, `7d4bf52f` shared authority and mutation barrier, and `a1292986`
  forced concurrency regression; evidence commit pending.

## Review policy and outcome

Authorized for automatic implementation and pull-request delivery by the
maintainer on 2026-11-13. Local static and unit evidence passes. Keep in
`gates-todo/` pending exact-head review and fresh Linux integration before any
merge.
