# Rocky Linux RPM packaging

## Source

Human direction on 2026-11-13 to make RPM packaging the gate following Gate
25's declarative operator CLI.

## Goal

Produce an installable, verifiable Ginux RPM and source RPM for Rocky Linux 10
x86-64 so an operator can install, upgrade, validate, run, and remove Ginux
through the native package manager without using the development-host copy
script.

## Approved scope

- Add a maintained RPM spec and source-manifest workflow for the Ginux CLI,
  freestanding runtime, privileged helper, example recipes, documentation,
  license, and notices.
- Move generic OCI acquisition, digest verification, layer caching, whiteout
  processing, and direct instance-root assembly into the compiled Rust control
  plane. Do not package OCI acquisition shell scripts or duplicate pinned-image
  metadata; each recipe's digest-qualified `instance.root` is authoritative.
- Package the compiled operator CLI as `/usr/bin/ginux` and keep runtime and
  privileged implementation programs under `/usr/libexec` with no public
  aliases.
- Replace production assumptions about `/usr/local` with canonical packaged
  paths while retaining explicit test overrides where fixtures require them.
- Create `/var/lib/ginux/cache/oci` and `/var/lib/ginux/instances` through a
  packaged tmpfiles declaration. OCI manifests, configuration objects, and
  compressed layers are retained by digest under `cache/oci`; each instance's
  fully expanded writable filesystem is
  `/var/lib/ginux/instances/NAME/root`. Package removal must not delete cached
  blobs, expanded roots, recipes, deployment state, logs, or other operator
  data.
- Assemble a new instance directly from verified cached OCI blobs into a
  staging root and atomically publish the instance directory. Do not persist a
  redundant assembled `rootfs.tar` beside the source OCI layers.
- Keep transient PIDs, FIFOs, and control endpoints under
  `/run/ginux/instances/NAME`, not in persistent state.
- Install representative example recipes under `/etc/ginux/examples` without
  overwriting operator changes during upgrade. Name every recipe
  `<distribution><major>-<purpose>.toml`, such as
  `rocky10-agent-workspace.toml` and `ubuntu24-network-service.toml`; determine
  and record the exact Alpine base before naming its recipe.
- Declare runtime and build dependencies explicitly; do not rely on whatever
  happens to be present in the development VM.
- Generate a binary RPM and SRPM from a clean source archive. Dependency
  acquisition, if required to assemble vendored Rust sources, must be an
  explicit preparation step; the actual RPM build must run without network
  access.
- Add deterministic Make targets and CI checks for package construction,
  contents, metadata, installation, verification, upgrade/reinstall behavior,
  removal, and state preservation.
- Exercise the installed package in a clean Rocky Linux 10 x86-64 QEMU host
  using only packaged paths and the public `ginux` CLI.
- Document package build, installation, upgrade, validation, removal, state
  retention, dependencies, and current platform limitations.

## Non-goals

- Publishing or operating a public RPM repository.
- Signing packages or managing release signing keys.
- Fedora, CentOS Stream, RHEL, AlmaLinux, or Rocky Linux versions other than
  Rocky Linux 10 in this first packaging gate.
- Architectures other than x86-64.
- Automatically starting a daemon or instance during installation.
- Storing system-wide images or instances in an invoking user's home directory.
- Installing a broad sudoers rule, weakening SELinux, changing firewall
  policy, or granting ordinary users new host privileges.
- Splitting Ginux into multiple subpackages before package contents and update
  cadence demonstrate a concrete need.
- Changing instance recipes, contract semantics, runtime behavior, or the
  public command taxonomy established by Gate 25.

## Completed work

- [x] Confirmed that the repository has no RPM spec or RPM build pipeline.
- [x] Confirmed that current development installation copies the CLI, runtime,
  helper, image tools, and one example recipe into `/usr/local` and `/etc`.
- [x] Drafted the initial package boundary, lifecycle behavior, and verification
  requirements.

## Remaining work

- [ ] Obtain human approval of the package scope, paths, platform target, and
  checkpoint policy after Gate 25 is accepted.
- [ ] Inventory every installed file, executable interpreter, shared-library
  dependency, external command, writable path, and ownership/mode requirement.
- [ ] Implement the source archive, vendored dependency, spec, tmpfiles, and
  package build workflow.
- [ ] Replace the installed shell OCI pipeline with compiled Rust acquisition
  and direct layer-to-instance-root assembly.
- [ ] Migrate production path defaults and the development installer to one
  authoritative layout without exposing private executables.
- [ ] Add package metadata, contents, permission, and dependency tests.
- [ ] Add clean install, package verification, reinstall/upgrade, functional
  instance workflow, erase, and persistent-state tests on Rocky Linux 10.
- [ ] Update user and maintainer documentation.
- [ ] Run all criterion-mapped verification and record fresh evidence.
- [ ] Push the dedicated branch, publish one pull request, and stop for human
  acceptance.

## Acceptance criteria

- `rpmbuild -ba` produces one noarch-independent x86-64 `ginux` binary RPM and
  its matching SRPM from a clean source archive without network access during
  the build.
- Package name, epoch/version/release, architecture, summary, description,
  license, source provenance, build requirements, runtime requirements, and
  changelog are present and machine-checkable; the RPM version is derived from
  the authoritative project version rather than duplicated silently.
- The RPM owns every shipped file and directory with reviewed modes and
  ownership. Public executables are under `/usr/bin`; private executables are
  under `/usr/libexec`; distribution-prefixed example recipes are under
  `/etc/ginux/examples`; documentation is in its canonical location; no
  production file is installed under `/usr/local`.
- No OCI acquisition shell script, separate image-pin environment file, or
  assembled `rootfs.tar` is installed or retained. The recipe's digest-qualified
  root reference selects content-addressed OCI blobs, and instance creation
  expands those blobs directly into the instance root with OCI whiteout and
  replacement semantics preserved.
- Persistent state is limited to `/var/lib/ginux/cache` and
  `/var/lib/ginux/instances`; each expanded instance root is stored under its
  named instance directory. Transient process and console-control state is
  under `/run/ginux/instances` and is recreated safely after reboot.
- Installation creates only required package and empty state structure. It
  does not pull an OCI image, create or start an instance, enable a service,
  add broad privilege rules, disable SELinux, or change firewall policy.
- `rpm -V ginux` passes immediately after installation, and packaged private
  commands do not appear in `ginux --help` or as public PATH aliases.
- `ginux --version`, global and per-command help, recipe validation, instance
  import/start/status/stop, and cached restart pass using only packaged files
  on a clean Rocky Linux 10 host.
- Reinstall or upgrade preserves modified configuration and every existing
  instance, image cache, overlay, lock state, deployment setting, and log.
- Erasing the package removes package-owned programs and immutable examples but
  never recursively deletes persistent Ginux state. A subsequent reinstall
  can see and operate retained compatible state.
- Missing declared runtime dependencies fail at package transaction resolution
  rather than as an unexplained post-install runtime failure.
- Package lint and repository naming, security-boundary, CLI, configuration,
  systemd, contract, security, and aggregate regressions remain green.
- Documentation gives exact commands for building the SRPM/RPM, inspecting
  contents, installing, verifying, upgrading, removing, and deliberately
  purging retained state.

## Verification plan

- `make rpm-source` -> creates versioned source and vendored-dependency
  archives with recorded checksums and no untracked repository changes.
- `make rpm` -> runs an offline `rpmbuild -ba` and produces the expected SRPM
  and x86-64 RPM or returns nonzero.
- `make test-rpm-metadata` -> verifies NEVRA, architecture, license, source,
  dependencies, scripts, file ownership, modes, canonical paths,
  distribution-prefixed recipe names, and absence of `/usr/local` payloads,
  OCI acquisition scripts, duplicate image-pin metadata, or public
  private-command aliases.
- `rpmlint <SRPM> <RPM>` -> returns no packaging errors; reviewed warnings, if
  any, are recorded with rationale.
- `make test-rpm` -> boots a clean Rocky Linux 10 x86-64 QEMU host; installs
  through DNF; runs `rpm -V`; exercises help, validation, import, start,
  status, stop, cache reuse, reinstall/upgrade, erase, retained-state checks,
  and reinstall recovery; exits nonzero on any mismatch.
- `make check test-cli test-config test-systemd test-contract test-security
  test` -> existing static, CLI, runtime, contract, security, and aggregate
  regressions remain green.
- `make demo-gate-26` -> builds the package, displays metadata and contents,
  installs it on the test host, runs one public packaged workflow, demonstrates
  state-preserving erase/reinstall, and reports cleanup instructions.

## Evidence

- Repository inspection -> no `.spec`, `rpmbuild`, or RPM package target exists.
- `scripts/install-ginux-host.sh` inspection -> current development installation
  copies the public CLI to `/usr/local/bin`, private components to
  `/usr/local/libexec`, and an example recipe to `/etc/ginux/examples`.
- `scripts/ginux-host-instance-helper.sh` inspection -> current persistent
  instances are fully expanded at `/var/lib/ginux/instances/NAME/root`, while
  the generic image pipeline also retains an assembled `rootfs.tar`; Gate 26
  removes that redundant intermediate and retains only verified OCI content
  blobs plus each instance root.
- Gate 25 pull request https://github.com/gmkurtzer/ginux/pull/273 -> supplies
  the compiled public CLI and explicit public/private executable boundary that
  packaging will consume after acceptance.

## Decisions and limitations

- Proposed initial deliverable is one `ginux` RPM plus SRPM for Rocky Linux 10
  x86-64. Subpackages and broader RPM-family compatibility are deferred until
  evidence demonstrates a need.
- The package manages software, immutable examples, and empty state-directory
  structure. Instance state and OCI caches are operator data and intentionally
  survive package erase.
- Example recipes are RPM configuration files under `/etc/ginux/examples` and
  use distribution-and-major-version filename prefixes.
- OCI acquisition is product functionality, not packaged shell tooling. Its
  Rust implementation shares the same typed image reference and error model as
  the public control plane while privileged state mutation remains narrowly
  separated.
- Package installation does not imply authorization to operate privileged
  Ginux functions. Existing host administrator authorization remains required.
- Gate 26 depends on Gate 25 acceptance because the RPM must package the final
  compiled public CLI and command boundary.

## Delivery strategy

- Working branch: `hdae/rpm-packaging`
- Intended base: `main` after Gate 25 is accepted and merged
- Pull request: intended after implementation and verification
- Planned commits: gate alignment and installed-file inventory; RPM source and
  spec implementation; canonical path and installation integration; package
  lifecycle tests; documentation and evidence reconciliation
- Actual commits: pending

## Review policy and outcome

Human checkpoint gate. Gate 26 must not begin implementation until Gate 25 is
accepted and merged and the human approves this package scope. After complete
verification, publish one dedicated pull request and stop for human acceptance.
Technical passage alone does not close the gate.
