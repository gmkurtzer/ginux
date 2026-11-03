# Virtual kernel views

## Source

Approved requirement that domains receive Ginus-created `/proc`, `/sys`, and
cgroup interfaces rather than host passthrough.

## Goal

Expose the minimum internally consistent virtual kernel state needed by the
Rocky systemd baseline while proving that host processes, devices, cgroups,
mounts, identifiers, and configuration are not leaked as domain authority.

## Approved scope

- Implement the tested `/proc` process, system, mount, memory, uptime, and
  identity files from Ginus state.
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

- [ ] None; gate not started.

## Remaining work

- [ ] Implement virtual procfs views.
- [ ] Implement virtual sysfs and assigned-device projection.
- [ ] Implement bounded cgroup v2 and mount/identity behavior.
- [ ] Add consistency, permission, mutation, and host-leakage tests.

## Acceptance criteria

- Managed process listings and status expose only Ginus tasks and use virtual
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

## Verification plan

- `make test-kernel-views` -> procfs, sysfs, mount, namespace, identity,
  consistency, permission, mutation, and host-leakage tests pass.
- `make test-cgroup` -> selected systemd cgroup v2 operations pass and host
  cgroup snapshots remain unchanged outside Ginus-owned resources.
- Host-versus-domain inventory comparison -> no unassigned host object appears
  in the managed fixture's observable results.
- `make test` with network disabled -> exits zero.

## Evidence

- Not run.

## Decisions and limitations

- Each virtual file is an explicit compatibility commitment; unimplemented
  paths must not silently fall through to the host.
- The cgroup profile is initially bounded to the accepted systemd baseline.

## Delivery strategy

- Working branch: `hdae/virtual-kernel-views`
- Intended base: `main`
- Pull request: intended; do not push or open without explicit authorization
- Planned commits: procfs model; sysfs and device projection; cgroup and
  mount/identity model; consistency and leakage verification
- Actual commits: pending

## Review policy and outcome

Human checkpoint because these interfaces define the kernel identity and
authority visible to the domain.

