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
- [x] Connect normalized device/storage/network configuration to the registry
  and exercise it in the Rocky QEMU host.
- [x] Implement the mediated file-backed block adapter and bounded read-write
  partition with flush and close policy.
- [x] Implement the bounded, isolated packet-queue network backend.

## Remaining work

- [x] No technical work remains; human checkpoint acceptance is required.

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
  passed before host integration.
- `make test-devices` -> passed in the Rocky Linux 10 QEMU TCG host. The
  normalized configuration projected six assigned devices; Ginus mediated a
  disposable file-backed 2048-byte partition, flushed and closed it, rejected
  an out-of-range write, kept the preceding 1024 bytes zero, exchanged an
  isolated packet, denied unknown controls/mapping/conflicting ownership, and
  released the backend for reuse.
- `make test-block` -> two range-confinement, read-only, flush, and close tests
  passed against disposable backends.
- `make test-network` -> isolated bounded send/receive, denied control, queue,
  and revocation test passed.
- `make bench-block` -> passed 1024 write/read pairs (524288 bytes, 2048
  operations) in Rocky x86-64 under QEMU TCG and recorded wall time, process
  CPU time, throughput, latency, workload, repetitions, bytes, and environment.
- `make bench-network` -> passed 1024 isolated send/receive pairs (131072
  bytes, 2048 operations) with the same required metadata.
- `make demo-gate-6` -> passed 6/6 named human checks and retained benchmark
  records in `build/bench-block.txt` and `build/bench-network.txt`.
- `make test` -> passed formatting, strict lint, 31 unit tests, prior Gate 1,
  2, 3, and 4 regressions, and the Gate 6 Rocky QEMU device path.

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
- Pull request: https://github.com/gmkurtzer/usshim/pull/6; open and awaiting
  review; never merge or close without explicit authorization
- Planned commits: virtual device registry and pseudo-devices; block and
  partition backend; network backend; isolation and benchmark verification
- Actual commits: `eb39ada` added the registry and pseudo-device foundation;
  `0ad0e90` added normalized configuration plus mediated block/network QEMU
  verification; `17568e3` added reproducible benchmarks, documentation, and
  the human demonstration

## Review policy and outcome

Human checkpoint because device mediation and backend ownership materially
affect the security boundary. Present `make demo-gate-6`, fresh evidence, and
the pull request before acceptance.
Technical status: passed and delivered on 2026-11-03. Human acceptance was
recorded on 2026-11-03; Gate 6 is closed.
