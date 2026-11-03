# OCI-backed virtual VFS and dynamic userspace

## Source

Rocky image pipeline and execution-kernel foundations.

## Goal

Virtually boot a digest-pinned Rocky OCI root filesystem through Ginus, then run
an unmodified dynamically linked Rocky Linux test program and shell against its
Ginus-owned immutable base and domain-local writable overlay.

## Approved scope

- Implement virtual path resolution, directories, regular files, symlinks,
  metadata, current root and working directory for the tested profile.
- Implement Ginus-owned descriptor tables, open-file objects, offsets,
  duplication, close-on-exec, and bounded read/write/stat operations.
- Pull a pinned Rocky OCI image only through an explicit acquisition target,
  verify and cache its manifest, configuration, and layers outside Git, and
  deterministically assemble the layers into an indexed immutable root.
- Apply OCI whiteout and opaque-directory semantics required by the selected
  image and present the assembled root with a disposable domain-local writable
  overlay.
- Consume OCI launch metadata while keeping Ginus as the execution runtime;
  do not invoke an external OCI runtime for managed code.
- Implement ELF interpreter loading and the minimum file and memory behavior
  needed by the selected Rocky dynamic program and shell fixtures.
- Prevent managed code from learning or receiving host paths and descriptors as
  authority.

## Non-goals

- Full Linux VFS, arbitrary filesystems, persistent production storage,
  package-manager compatibility, or host bind mounts.
- `/proc`, `/sys`, cgroups, general devices, and networking.
- OCI runtime lifecycle compatibility, OCI hooks, runtime-spec namespaces, or
  delegation to `runc`, `crun`, `containerd`, or another execution runtime.
- Direct use of host pathnames supplied by managed code without Ginus lookup
  and policy.

## Completed work

- [x] Selected the initial backing model with human approval: a digest-pinned
  Rocky OCI image is the virtual domain root; Ginus caches and assembles it,
  virtually boots its userspace, and remains the execution runtime.
- [x] Implemented the initial immutable index, virtual path/object, symlink,
  descriptor, and in-memory overlay foundations.

## Remaining work

- [ ] Implement explicit OCI acquisition, digest verification, cache identity,
  deterministic layer assembly, and whiteout handling.
- [ ] Connect the verified OCI root and writable overlay to the domain.
- [ ] Add dynamic ELF interpreter and library loading support.
- [ ] Add isolation, traversal, overlay, malformed-file, and cleanup tests.

## Acceptance criteria

- The selected unmodified Rocky dynamic fixture and shell start with their
  image-provided loader and libraries.
- Reads observe immutable base content; writes affect only the domain overlay
  and do not modify cached OCI manifests, configuration, or layers.
- Ordinary domain start and the complete offline test suite perform no registry
  access; a missing or corrupt cached object fails closed with a useful error.
- Managed execution is performed by Ginus, and the demonstration verifies that
  no external OCI runtime launches the domain workload.
- Path traversal, symlink loops, invalid descriptors, permission failures, and
  overlay deletion behave deterministically for the documented profile.
- Managed descriptor values cannot be used as host descriptors, and host paths
  are absent from the managed ABI and audit output unless explicitly sanitized.
- Domain teardown releases open objects and removes its disposable overlay.
- Earlier runtime, configuration, image, and interception tests remain passing.
- `make demo-gate-5` lets a human run the selected Rocky dynamic program and
  shell, modify the overlay, and verify the immutable base remains unchanged.

## Verification plan

- `make test-vfs` -> path, descriptor, overlay, isolation, malformed-input, and
  teardown tests pass.
- `make test-runtime` -> static and selected dynamic Rocky fixtures pass.
- Cached OCI-object integrity comparison before and after VFS tests ->
  unchanged.
- `make test` with network disabled -> exits zero.
- `make demo-gate-5` -> performs the documented dynamic-userspace and overlay
  isolation demonstration, cleans up, and exits zero.

## Evidence

- Not run.

## Decisions and limitations

- Compatibility is bounded to documented fixtures and semantics; unsupported
  filesystem operations must fail explicitly rather than silently use host
  behavior.
- Durable writable domain storage is deferred.
- The trusted Rocky host can name the OCI cache and index. Managed code can
  name only virtual paths and receives no host path or descriptor authority.
- The Rocky cloud QCOW2 boots only the host and is not the domain root backing.
  Domain writes use a bounded in-memory overlay and cannot modify cached OCI
  objects or the immutable assembled root.

## Delivery strategy

- Working branch: `hdae/virtual-vfs-userspace`
- Intended base: `main`
- Pull request: intended; dedicated branch push and PR creation authorized on
  2026-11-03; never merge or close without explicit authorization
- Planned commits: virtual path and descriptor model; OCI acquisition and
  immutable root assembly; domain overlay; dynamic loader compatibility and
  isolation tests
- Actual commits: `4994fed` recorded the superseded derived-QCOW decision;
  `c6e3124` implemented the reusable indexed VFS core; correction commit pending

## Review policy and outcome

Human checkpoint because host-backed files are a security boundary and the
accepted VFS model constrains later device and systemd behavior. Present
`make demo-gate-5`, fresh evidence, and the pull request before acceptance.
