# Scope freestanding Rust flags to freestanding builds

## Source

[Issue #318](https://github.com/gmkurtzer/ginux/issues/318), reporting that
target-wide x86-64 GNU Rust flags contaminate host build scripts when the host
and target triples are identical.

## Goal

Keep ordinary host and workspace Cargo units on the host toolchain while
applying the freestanding linker contract only to explicit Ginux freestanding
binary builds.

## Approved scope

- Remove x86-64 GNU freestanding flags from repository-wide Cargo target
  configuration.
- Apply the existing flags to the explicit `ginux-init`, managed payload,
  baseline, runtime fixture, and process fixture build and lint commands.
- Add Linux CI evidence that ordinary workspace host units compile without the
  freestanding-only packages or linker contract before the explicit
  freestanding build runs.

## Non-goals

- Do not change the public CLI target or static-musl linking.
- Do not change any freestanding binary ABI, entry point, relocation model, or
  runtime behavior.
- Do not introduce a custom Rust target specification unless command-scoped
  flags prove insufficient.

## Completed work

- [x] Confirmed target-wide flags apply to host units on x86-64 Linux because
      the configured target equals the host triple.
- [x] Removed the target-wide GNU Rust flags and scoped the unchanged flag set
      to explicit freestanding Makefile invocations.
- [x] Added an x86-64 Linux workspace-host compile before the freestanding CI
      build.

## Remaining work

- [x] Run local host-package, lint, unit, and canonical build checks.
- [ ] Run exact-head Linux CI.
- [ ] Deliver the verified branch in a linked pull request and add reciprocal
      issue/PR comments.

## Acceptance criteria

- Ordinary Cargo host crates and build scripts do not receive `rust-lld`,
  `-nostdlib`, `-static`, or `-no-pie` from repository configuration.
- Every freestanding binary still receives the complete existing linker and
  static-relocation flag set in canonical Make workflows.
- A workspace check excluding the five freestanding-only packages succeeds on
  x86-64 Linux before their explicit target builds.
- `make check`, `make test-unit`, and `make build` retain their existing
  behavior and outputs.

## Verification plan

- `cargo check --workspace --all-targets` with the five freestanding-only
  packages excluded -> ordinary host packages and build scripts compile without
  target-wide GNU flags.
- `make check` -> host lints and explicit freestanding lints pass.
- `make test-unit` -> component tests pass without inherited freestanding flags.
- `make build` -> every canonical deliverable still links.
- Fresh exact-head Linux CI -> host workspace check, freestanding build, and
  launch integration all pass.

## Evidence

- `cargo check --workspace --all-targets` with the five freestanding-only
  packages excluded -> passed; ordinary host dependencies and build scripts
  compile without target-wide GNU flags.
- `make check` -> passed; every ordinary lint and every explicitly scoped
  freestanding lint passed.
- `make test-unit` -> passed all component and oracle tests.
- `make build` -> passed all six canonical Rust deliverables with the unchanged
  freestanding flag set applied only to the five GNU-target binaries.
- Development host: macOS arm64; exact x86-64 Linux host behavior remains for
  pull-request CI.

## Decisions and limitations

- Command-scoped `RUSTFLAGS` is intentionally used only with explicit
  `--target` freestanding package invocations. This is smaller and easier to
  audit than maintaining a custom target specification.

## Delivery strategy

- Working branch: `hdae/scope-freestanding-rustflags`
- Intended base: `main`
- Pull request: intended; it will close issue #318 after exact-head review and
  merge.
- Planned commits: scoped build configuration and Linux regression; HDAE
  evidence and reciprocal links.
- Actual commits: `f845cbf9` scopes the existing freestanding flags, adds the
  Linux host-package regression, and records the local evidence.

## Review policy and outcome

Implementation and pull-request delivery are authorized by the repository's
proactive-remediation policy. Keep the gate in `gates-todo/` until exact-head
Linux review and merge.
