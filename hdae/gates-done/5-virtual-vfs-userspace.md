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
- Consume OCI launch metadata while keeping Ginus as the supervising userspace
  kernel; do not invoke an external OCI runtime for managed code.
- Pass the selected ELF and image-provided dynamic loader to the host kernel for
  native execution under continuous ptrace syscall-entry/exit supervision.
  Every syscall must stop before host effect and resolve through Ginus dispatch.
- Implement the approved balanced profile: native instruction execution with
  confined passthrough only for the minimal read-only loader, immutable-image,
  memory, signal, and bootstrap-console surface. Writable storage, devices,
  networking, ioctls, credentials, and virtual kernel state remain emulated,
  proxied through Ginus objects, or denied.
- Prevent managed code from learning host paths or receiving supervisor and
  unrelated host descriptors as authority.

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
- [x] Implemented explicit OCI acquisition and offline verification for the
  pinned Rocky 10.2 linux/amd64 manifest, configuration, and layer.
- [x] Verified the uncompressed OCI `diff_id`, built the Ginus root index, and
  rejected missing and corrupt cache objects.
- [x] Connected the OCI root and disposable write overlay to Ginus in the
  emulated Rocky host; verified Rocky identity, Bash ELF access, teardown, and
  rejection of a truncated VFS index.

## Remaining work

- [x] Generalized deterministic assembly across ordered OCI layers and added
  synthetic replacement, whiteout, opaque-directory, and repeatability tests.
- [x] Enabled the native `ptrace-native` backend and proved the image-provided
  dynamic loader, libraries, and Bash execute from a read-only OCI bootstrap
  view with entry/exit audit and fail-closed contract decisions.
- [x] Enforced and tested the approved balanced boundary: kernel descriptors are
  confined to immutable OCI content and the assigned bootstrap console;
  sensitive interfaces cannot use native passthrough.
- [x] Added a fail-closed supervisor-loss test and explicit audit assertions for
  emulated and denied syscalls before effect.
- [x] Added unexpected-stop and remaining ptrace-state tests.
- [x] Added isolation, traversal, overlay, malformed-file, and cleanup tests.
- [x] Added `make demo-gate-5` with eight named human-readable checks and
  explicit non-claims.
- [x] Delivered the dedicated branch and pull request.
- [ ] Obtain human acceptance at the Gate 5 checkpoint.

## Acceptance criteria

- The selected unmodified Rocky dynamic fixture and shell start with their
  image-provided loader and libraries.
- Reads observe immutable base content; writes affect only the domain overlay
  and do not modify cached OCI manifests, configuration, or layers.
- Ordinary domain start and the complete offline test suite perform no registry
  access; a missing or corrupt cached object fails closed with a useful error.
- Managed execution is performed by Ginus, and the demonstration verifies that
  no external OCI runtime launches the domain workload.
- The host kernel loads and natively executes the selected ELF, but every
  managed syscall is observed and decided before effect; only explicitly
  validated passthrough operations may execute in the native backend.
- Tracee escape, unexpected ptrace state, or supervisor loss terminates the
  domain without allowing an unsupervised workload to continue.
- Path traversal, symlink loops, invalid descriptors, permission failures, and
  overlay deletion behave deterministically for the documented profile.
- Under the balanced profile, kernel descriptor values are usable only through
  validated operations on immutable OCI content or the assigned bootstrap
  console inside the private domain view. Ginus/supervisor descriptors and host
  paths are absent from the managed ABI and sanitized audit output.
- Writable storage, devices, networking, ioctls, credentials, and virtual
  kernel state do not use native passthrough in the balanced profile.
- Domain teardown releases open objects and removes its disposable overlay.
- Earlier runtime, configuration, image, and interception tests remain passing.
- `make demo-gate-5` lets a human run the selected Rocky dynamic program and
  shell, modify the overlay, and verify the immutable base remains unchanged.

## Verification plan

- `make test-vfs` -> path, descriptor, overlay, isolation, malformed-input, and
  teardown tests pass.
- `make test-runtime` -> static and selected dynamic Rocky fixtures pass.
- Balanced-profile negative tests -> mutation flags, ioctls, networking,
  credential changes, and unknown operations are denied before host effect.
- Cached OCI-object integrity comparison before and after VFS tests ->
  unchanged.
- `make test` with network disabled -> exits zero.
- `make demo-gate-5` -> performs the documented dynamic-userspace and overlay
  isolation demonstration, cleans up, and exits zero.

## Evidence

- `make test-oci-image` passed seven checks: pinned object digests and sizes,
  verified `diff_id`, repeatable Ginus index, Rocky/Bash identity, corrupt and
  missing cache rejection, offline verification/preparation, and synthetic
  multi-layer replacement, whiteout, and opaque-directory semantics.
- The pinned linux/amd64 OCI manifest is
  `sha256:e372170ca8630f0f03e9b70fdd0bf4a3ce3426b0de7cdba615f06337389de176`;
  its single layer expands to verified `diff_id`
  `sha256:f0c7ff1acfc99a513a834c43874f79b8584ae074e5e56fa6aafc3dcf4f5ab428`.
- `make test-vfs` passed inside the QEMU TCG Rocky host: Ginus resolved
  `/etc/os-release` and `/usr/bin/bash` from the OCI root, created and removed
  an overlay-only file, exposed no managed host path or descriptor, and
  rejected a truncated index.
- `make check` and `make test-unit` passed after the OCI/VFS integration.
- `make test-native-vfs` passed in the QEMU TCG Rocky host. The host loaded the
  unmodified OCI Bash and its image-provided dynamic loader, Ginus audited 238
  syscall entries, emulated `getpid` as 4242, denied unsupported calls, observed
  permitted exits, printed `GINUS_NATIVE_SHELL: PASS`, and stopped the tracee
  at `exit_group`. The Bash file digest was unchanged afterward.
- After multi-layer assembly commit `19ade45`, `make check`, `make test-unit`,
  `make test-oci-layers`, and `make test-oci-image` passed. A fresh
  `make test-native-vfs` then passed against the assembled root in QEMU.
- The expanded `make test-native-vfs` started a held Rocky Bash tracee, killed
  the Ginus supervisor with `SIGKILL`, and verified `PTRACE_O_EXITKILL` removed
  the tracee. The same run asserted virtual `getpid` and denied `getppid` audit
  entries and passed normal cleanup.
- After balanced-profile approval, `make check` and `make test-unit` passed the
  new profile/configuration, syscall-surface classification, mutation-flag,
  W+X, ioctl, networking, credential-change, and unknown-syscall checks.
  `make test-native-vfs` then passed in QEMU with classified immutable-image and
  bootstrap-console passthrough, virtual PID/UID/GID/parent identity, denied
  ioctl, a denied `O_CREAT` probe with no resulting file, a denied socket probe,
  and the existing supervisor-loss check.
- The expanded QEMU run sent `SIGSTOP` to a held tracee. Ginus treated the
  unexpected ptrace stop as fatal, killed and reaped the tracee, reported the
  exact failure, and released domain state. The VFS unit suite also passed a
  two-instance overlay-isolation test alongside traversal, symlink-loop,
  invalid-descriptor, permission, unlink, malformed-index, and teardown checks.
- `make demo-gate-5` passed 8/8 checks: pinned offline OCI objects, multi-layer
  semantics, Ginus VFS overlay/teardown, unmodified Rocky loader and Bash,
  balanced allow/deny policy, virtual identity, fail-closed tracing, unchanged
  cached root/index, and cleanup. It explicitly excludes full VFS, devices,
  networking, `/proc`, `/sys`, and systemd claims.
- A fresh aggregate `make test` passed formatting/lint, 24 unit tests, Gate 1
  interception, Gate 3 runtime, Gate 4 configuration/lifecycle, and all eight
  Gate 2 image-pipeline checks.

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
- The selected Rocky image currently has one filesystem layer, so general
  multi-layer behavior is covered by deterministic synthetic layer fixtures.
- Gate 5 uses native host ELF execution under ptrace syscall-entry/exit
  supervision. `PTRACE_SYSEMU` remains useful for the Gate 1 no-effect proof,
  but cannot execute an approved syscall. The native path deliberately exposes
  the host ELF loader, memory manager, faults, signals, scheduler, ptrace, and
  explicitly validated syscall implementations; Ginus does not claim a
  userspace-only CPU or memory boundary on this path.
- The current Bash proof uses a read-only, unprivileged chroot bootstrap view.
  Human direction on 2026-11-03 approved evolving it into the balanced profile:
  read-only immutable-image and bootstrap-console kernel descriptors are
  permitted only inside a private domain view, while sensitive operations must
  remain Ginus-mediated or denied.
- Execution backend and syscall action are orthogonal. The security profile
  retains the Gate 3 interpreter and Ginus-virtual descriptors; the balanced
  profile is Gate 5's compatibility path; a broader performance profile
  requires later benchmarks and review. Per-process backend selection is
  deferred beyond this gate.
- Unicorn Engine 2 is preserved as deferred Gate 14. It is the alternative path
  for moving CPU and memory execution back into Ginus without discarding the
  OCI, VFS, contract, or lifecycle work from this gate.

## Delivery strategy

- Working branch: `hdae/virtual-vfs-userspace`
- Intended base: `main`
- Pull request: https://github.com/gmkurtzer/usshim/pull/5; open and awaiting
  review; never merge or close without explicit authorization
- Planned commits: virtual path and descriptor model; OCI acquisition and
  immutable root assembly; domain overlay; dynamic loader compatibility and
  isolation tests
- Actual commits: `4994fed` recorded the superseded derived-QCOW decision;
  `c6e3124` implemented the reusable indexed VFS core; `2b111fc` corrected the
  root architecture to OCI; `287127e` added pinned OCI acquisition and root
  preparation; `b1a320e` connected the OCI root to Ginus and QEMU verification;
  `8e5847a` added the native ptrace Rocky loader and Bash proof; `19ade45`
  added deterministic multi-layer OCI assembly; `43db983` added supervisor-loss
  protection; `1c18c36` added the approved balanced execution profile;
  `f151475` added unexpected-stop and final isolation checks; `bba01f9` added
  the human demonstration and final technical evidence; subsequent ledger
  commits recorded delivery in pull request 5

## Review policy and outcome

Human checkpoint because host-backed files are a security boundary and the
accepted VFS model constrains later device and systemd behavior. Present
`make demo-gate-5`, fresh evidence, and the pull request before acceptance.
Technical status: passed and delivered on 2026-11-03. Human acceptance was
recorded on 2026-11-03; Gate 5 is closed.
