# Device ownership and virtual hardware

## Source

Approved requirement for domain-assigned block devices, bounded partitions,
virtual network devices, and Ginus-owned `/dev`.

## Goal

Give one domain an explicit, exclusive set of Ginus virtual devices without
exposing host descriptors, device nodes, unchecked ioctls, mappings, MMIO, or
DMA authority.

## Approved scope

- Implement the initial Ginus `/dev` tree with console, null, zero, and random
  device behavior needed by the validated userspace.
- Implement a file-backed virtual block device and a bounded partition view
  with explicit size, read-only/read-write, flush, and lifecycle policy.
- Implement one virtual network interface with a documented mediated backend.
- Derive device visibility and names exclusively from normalized domain
  configuration.
- Enforce exclusive assignment by default and reject conflicting live-domain
  ownership.
- Decode, validate, and explicitly implement admitted ioctl families; deny
  unknown requests.
- Deny direct device mappings unless Ginus owns and implements the mapped
  memory semantics.
- Add block and network functional and benchmark targets.

## Non-goals

- Physical disks, production partitions, GPUs, arbitrary host `/dev`, VFIO,
  direct MMIO, DMA, SR-IOV, or hardware passthrough.
- Full Linux block, network, udev, or ethtool compatibility.
- Device hotplug while a domain is running.

## Completed work

- [x] Implement the bounded virtual device registry, Ginus-only descriptor
  namespace, assignment visibility, exclusive backend ownership, revocation,
  ioctl validation, and direct-mapping denial.
- [x] Implement Ginus-owned console, null, zero, and deterministic random
  frontends with unit verification.

## Remaining work

- [ ] Implement file-backed block and bounded partition behavior.
- [ ] Implement the approved virtual network backend.
- [ ] Add exclusivity, ioctl, mapping, teardown, and benchmark verification.

## Acceptance criteria

- The domain sees only devices assigned by its normalized configuration.
- A file-backed block device and bounded partition cannot access bytes outside
  their configured backing range.
- Conflicting assignment of an exclusive backend to two live domain records is
  rejected before the second domain starts.
- A managed device descriptor is not a host descriptor and cannot be used to
  address an unassigned host object.
- Admitted ioctls validate copied payloads; unknown or malformed requests fail
  without invoking an unchecked host ioctl.
- Direct host device memory and DMA are unavailable in the tested profile.
- Stop and failure paths flush according to policy, revoke assignment, close
  backends, and leave the disposable test devices reusable.
- Block and network benchmarks record throughput, latency, CPU, workload,
  repetitions, and environment without asserting an unapproved budget.
- `make demo-gate-6` lets a human inspect assigned devices, exercise block and
  network paths, and observe rejection of a conflicting assignment and ioctl.

## Verification plan

- `make test-devices` -> visibility, pseudo-device, console, assignment,
  conflict, ioctl, mapping-denial, and teardown tests pass.
- `make test-block` -> range confinement, read-only policy, read/write, flush,
  malformed request, and cleanup tests pass against disposable files.
- `make test-network` -> interface isolation, packet send/receive, denied
  control operation, and cleanup tests pass on an isolated test backend.
- `make bench-block` and `make bench-network` -> emit reproducible measurement
  records with environment metadata.
- `make test` with network access disabled except the isolated local test
  backend -> exits zero.
- `make demo-gate-6` -> performs the documented device assignment, I/O,
  conflict, denial, and cleanup demonstration and exits zero.

## Evidence

- `cargo test -p ginus-devices` -> six registry, pseudo-device, block,
  network, ownership, ioctl, mapping, and lifecycle tests passed.
- `cargo clippy -p ginus-devices --all-targets -- -D warnings` -> passed.
- `make test-devices test-block test-network` -> initial component targets
  passed; block remains memory-backed until the next milestone and benchmark
  targets are not yet implemented.

## Decisions and limitations

- Physical-device assignment requires a later threat-model and architecture
  gate; this gate must not create a generic raw-device escape hatch.
- The initial network backend is selected and documented during this gate
  without allowing an unmediated host syscall to complete.
- The initial network backend is a bounded Ginus-owned packet queue. It has no
  host socket, TUN/TAP descriptor, external connectivity, or unchecked control
  path. This establishes the mediated frontend; later connectivity is outside
  this gate.

## Delivery strategy

- Working branch: `hdae/device-ownership`
- Intended base: `main`
- Pull request: intended; dedicated branch push and PR creation authorized on
  2026-11-03; never merge or close without explicit authorization
- Planned commits: virtual device registry and pseudo-devices; block and
  partition backend; network backend; isolation and benchmark verification
- Actual commits: registry and pseudo-device foundation pending commit

## Review policy and outcome

Human checkpoint because device mediation and backend ownership materially
affect the security boundary. Present `make demo-gate-6`, fresh evidence, and
the pull request before acceptance.
