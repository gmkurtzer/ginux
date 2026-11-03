# Virtual VFS and dynamic userspace

## Source

Rocky image pipeline and execution-kernel foundations.

## Goal

Run an unmodified dynamically linked Rocky Linux test program and shell against
a Ginus-owned virtual filesystem backed by the prepared Rocky base and a
domain-local writable overlay.

## Approved scope

- Implement virtual path resolution, directories, regular files, symlinks,
  metadata, current root and working directory for the tested profile.
- Implement Ginus-owned descriptor tables, open-file objects, offsets,
  duplication, close-on-exec, and bounded read/write/stat operations.
- Present the prepared Rocky base as immutable content with a disposable
  domain-local writable overlay.
- Implement ELF interpreter loading and the minimum file and memory behavior
  needed by the selected Rocky dynamic program and shell fixtures.
- Prevent managed code from learning or receiving host paths and descriptors as
  authority.

## Non-goals

- Full Linux VFS, arbitrary filesystems, persistent production storage,
  package-manager compatibility, or host bind mounts.
- `/proc`, `/sys`, cgroups, general devices, and networking.
- Direct use of host pathnames supplied by managed code without Ginus lookup
  and policy.

## Completed work

- [ ] None; gate not started.

## Remaining work

- [ ] Implement virtual objects, path lookup, and descriptor semantics.
- [ ] Connect the verified Rocky base and writable overlay.
- [ ] Add dynamic ELF interpreter and library loading support.
- [ ] Add isolation, traversal, overlay, malformed-file, and cleanup tests.

## Acceptance criteria

- The selected unmodified Rocky dynamic fixture and shell start with their
  image-provided loader and libraries.
- Reads observe immutable base content; writes affect only the domain overlay
  and do not modify the cached verified source.
- Path traversal, symlink loops, invalid descriptors, permission failures, and
  overlay deletion behave deterministically for the documented profile.
- Managed descriptor values cannot be used as host descriptors, and host paths
  are absent from the managed ABI and audit output unless explicitly sanitized.
- Domain teardown releases open objects and removes its disposable overlay.
- Earlier runtime, configuration, image, and interception tests remain passing.

## Verification plan

- `make test-vfs` -> path, descriptor, overlay, isolation, malformed-input, and
  teardown tests pass.
- `make test-runtime` -> static and selected dynamic Rocky fixtures pass.
- Cached-image integrity comparison before and after VFS tests -> unchanged.
- `make test` with network disabled -> exits zero.

## Evidence

- Not run.

## Decisions and limitations

- Compatibility is bounded to documented fixtures and semantics; unsupported
  filesystem operations must fail explicitly rather than silently use host
  behavior.
- Durable writable domain storage is deferred.

## Delivery strategy

- Working branch: `hdae/virtual-vfs-userspace`
- Intended base: `main`
- Pull request: intended; do not push or open without explicit authorization
- Planned commits: virtual path and descriptor model; Rocky base and overlay;
  dynamic loader compatibility and isolation tests
- Actual commits: pending

## Review policy and outcome

Human checkpoint because host-backed files are a security boundary and the
accepted VFS model constrains later device and systemd behavior.

