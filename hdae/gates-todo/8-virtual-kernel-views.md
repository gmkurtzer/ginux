# Virtual kernel views

## Source

Approved requirement that domains receive Ginux-created `/proc`, `/sys`, and
cgroup interfaces rather than host passthrough.

## Goal

Expose the minimum internally consistent virtual kernel state needed by the
Rocky systemd baseline while proving that host processes, devices, cgroups,
mounts, identifiers, and configuration are not leaked as domain authority.

## Approved scope

- Implement the tested `/proc` process, system, mount, memory, uptime, and
  identity files from Ginux state.
- Implement the tested `/sys` device and kernel attribute tree from assigned
  virtual devices and contract-visible state.
- Implement the bounded cgroup v2 hierarchy and operations required by the
  selected systemd configuration.
- Implement the tested mount, namespace, hostname, machine identity, and boot
  identity views.
- Ensure all views remain consistent with virtual PIDs, devices, resources,
  filesystems, and lifecycle.
- Add explicit host-leakage and mutation-negative tests.

## Non-goals

- Bind-mounting host `/proc`, `/sys`, `/dev`, or cgroup trees.
- Full Linux procfs, sysfs, namespace, mount, or cgroup compatibility.
- General container nesting or delegated cgroup administration beyond the
  tested systemd profile.

## Completed work

- [x] Add the no-std `ginux-kernel-views` component and workspace/test
  integration.
- [x] Implement bounded virtual procfs task status/stat, PID listing, uptime,
  memory, mounts, hostname, and boot-identity reads from Ginux-owned inputs.
- [x] Add process-table snapshot projection plus negative host-PID, invalid-path,
  read-only mutation, buffer-boundary, and consistency tests.
- [x] Implement virtual sysfs class views from domain-assigned device snapshots,
  including virtual major/minor, network type, uevent, and CPU-online data.
- [x] Add explicit unassigned-device exclusion and sysfs mutation-denial tests.
- [x] Implement a bounded cgroup v2 hierarchy with root-only create/remove,
  controller enablement, task attachment, memory/PID limits, events, and
  explicit rejection of unsupported host-affecting operations.
- [x] Implement domain-owned machine/boot/hostname and mount/PID/UTS/cgroup
  namespace identity views with host-PID exclusion.
- [x] Add the Rocky QEMU integration fixture with host cgroup, mount, and block
  inventory before/after comparison.
- [x] Add stable `make test-kernel-views`, `make test-cgroup`, aggregate
  wiring, and the seven-check human demonstration.
- [x] Run the fresh aggregate offline regression suite.

## Remaining work

- [ ] Push the dedicated branch, open its pull request, and stop for human
  review.

## Acceptance criteria

- Managed process listings and status expose only Ginux tasks and use virtual
  identities consistent with the process runtime.
- Sysfs exposes only assigned virtual devices and approved attributes.
- Cgroup operations required by the systemd fixtures succeed inside the
  virtual hierarchy without mutating host cgroups.
- Host-only PIDs, mount paths, devices, kernel pointers, identifiers, and
  cgroup paths are absent from the tested domain views.
- Attempts to mutate unsupported or host-owned settings fail explicitly and
  leave host state unchanged.
- Machine, boot, hostname, mount, resource, and device information is
  internally consistent across the virtual interfaces.
- Earlier process, device, VFS, and runtime suites remain passing.
- `make demo-gate-8` lets a human inspect virtual `/proc`, `/sys`, and cgroups,
  compare them with host inventory, and observe rejection of a host mutation.

## Verification plan

- `make test-kernel-views` -> procfs, sysfs, mount, namespace, identity,
  consistency, permission, mutation, and host-leakage tests pass.
- `make test-cgroup` -> selected systemd cgroup v2 operations pass and host
  cgroup snapshots remain unchanged outside Ginux-owned resources.
- Host-versus-domain inventory comparison -> no unassigned host object appears
  in the managed fixture's observable results.
- `make test` with network disabled -> exits zero.
- `make demo-gate-8` -> performs the documented virtual-view, host-leakage, and
  mutation demonstration and exits zero.

## Evidence

- Initial `cargo test -p ginux-kernel-views` -> one assertion used an
  incorrect fixed window width for the expected PPid line; implementation
  output was correct and the assertion was fixed.
- Corrected `cargo test -p ginux-kernel-views` -> two virtual procfs,
  identity, mount, host-PID exclusion, and mutation-denial tests passed.
- `cargo clippy -p ginux-kernel-views --all-targets -- -D warnings` -> passed.
- `cargo test -p ginux-process` -> all seven prior process-runtime tests
  passed after adding bounded task snapshot projection.
- Expanded `cargo test -p ginux-kernel-views` -> three procfs/sysfs tests
  passed, including assigned block/network projection, virtual CPU inventory,
  unassigned host-device exclusion, and mutation denial.
- `cargo test -p ginux-devices` -> all six device ownership and isolation
  regressions passed after adding bounded device snapshot projection.
- Expanded `cargo test -p ginux-kernel-views` -> six tests passed, including
  bounded systemd-style cgroup operations, privilege/limit/busy-path failures,
  machine identity, consistent namespace links, host-PID exclusion, and
  read-only hostname denial.
- Strict clippy for `ginux-kernel-views` -> passed after the cgroup and
  identity expansion.
- `make check` -> passed name validation, formatting, and strict workspace
  lint including the new component and x86-64 static Ginux binary.
- Initial x86-64 static build exposed an implicit `memmove` dependency from
  cgroup member compaction. The compaction is now an explicit bounded loop;
  the static build and strict lint pass without adding host-runtime linkage.
- `make test-kernel-views` -> six component tests and the Rocky QEMU fixture
  passed; virtual procfs/sysfs/cgroup/identity markers matched and host
  cgroup, mount, and block-device inventories were byte-identical before/after.
- `make demo-gate-8` -> passed all seven human-visible virtual-view,
  leakage-denial, mutation-denial, and host-unchanged checks.
- Fresh aggregate `make test` -> passed formatting, strict lint, 44 component
  tests, all prior QEMU interception/runtime/config/domain/image/device/process
  regressions, and the Gate 8 Rocky QEMU kernel-view fixture.

## Decisions and limitations

- Each virtual file is an explicit compatibility commitment; unimplemented
  paths must not silently fall through to the host.
- The cgroup profile is initially bounded to the accepted systemd baseline.

## Delivery strategy

- Working branch: `hdae/virtual-kernel-views`
- Intended base: `main`
- Pull request: intended; dedicated branch push and PR creation authorized on
  2026-11-03; never merge or close without explicit authorization
- Planned commits: procfs model; sysfs and device projection; cgroup and
  mount/identity model; consistency and leakage verification
- Actual commits: the procfs foundation is recorded by the commit containing
  this evidence update

## Review policy and outcome

Human checkpoint because these interfaces define the kernel identity and
authority visible to the domain. Gate 8 is technically passed; branch delivery
and the pull request remain before explicit acceptance.
