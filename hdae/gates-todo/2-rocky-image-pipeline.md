# Rocky domain-userspace pipeline

## Source

Approved initial baseline using an official Rocky Linux 10 cloud QCOW2 image
without downloading it on every boot.

## Goal

Reproducibly prepare the Gate 1 verified and cached Rocky Linux 10 x86-64 image
content as a domain userspace that Ginus builds and tests can consume offline.

## Approved scope

- Reuse the official Rocky Linux 10 x86-64 cloud QCOW2 artifact and verified
  cache established for the Gate 1 host.
- Implement explicit preparation and domain-image test targets.
- Store downloaded and prepared artifacts in documented cache locations outside
  Git while keeping a small source manifest in Git.
- Prepare the filesystem content needed by Ginus without booting the image's
  Linux kernel.
- Define clean versus cache-removal behavior so ordinary cleanup does not force
  a redownload.
- Make all non-fetch verification usable with networking disabled.

## Non-goals

- OCI registry pulls, layer assembly, or boot-time downloads.
- Running the Rocky guest kernel as the workload boundary.
- General image conversion for arbitrary distributions.
- Implementing Ginus VFS semantics.

## Completed work

- [x] Validated the Gate 1 source manifest and XFS/GPT source layout.
- [x] Selected a pinned, network-disabled Docker/LinuxKit extraction helper so
  the Rocky source kernel is never booted during preparation.
- [x] Defined the rootfs tar, deterministic content manifest, cache ownership,
  and explicit cleanup contract.
- [x] Implemented pinned-helper acquisition, verified QCOW2 conversion,
  read-only GPT/XFS extraction, atomic cache publication, and cache reuse.
- [x] Added Rocky identity, systemd, loader, machine-identity, corruption,
  missing-source, repeatability, offline-network, and clean-preservation checks.
- [x] Documented artifact ownership, cleanup, offline operation, and the human
  demonstration.
- [x] Revised the human demonstration to expose eight named checks, concrete
  compared values, enforced extraction isolation, and explicit non-coverage.

## Remaining work

- [ ] Human runs or reviews `make demo-gate-2` and accepts or rejects the gate.

## Acceptance criteria

- The source manifest identifies one immutable official Rocky artifact and
  verifies it against its published cryptographic checksum.
- A corrupt or substituted download is rejected before preparation.
- A successful initial fetch permits repeated preparation and image tests with
  network access disabled.
- Repeated preparation from the same verified source produces the same
  content manifest or a documented deterministic equivalent.
- `make clean` preserves the verified cache; an explicit destructive cache
  target is documented separately and is not part of normal testing.
- No downloaded QCOW2, prepared root, or large generated artifact is tracked by
  Git.
- `make demo-gate-2` lets a human verify the pinned image, inspect its identity,
  prepare it offline from cache, and observe corruption rejection.

## Verification plan

- `make host-image-verify` -> revalidates the Gate 1 cached source and exits
  nonzero for a controlled corrupt copy.
- `make image-prepare` -> produces the documented prepared userspace and content
  manifest.
- `make test-image` -> validates required systemd, loader, library, and machine
  identity inputs in the prepared result.
- `make image-prepare test-image` with networking disabled after the first fetch
  -> exits zero without contacting a remote service.
- Repeat preparation -> content manifests compare equal under the documented
  determinism rules.
- `make demo-gate-2` -> performs the documented cache, identity, offline
  preparation, and corruption checks and exits zero.

## Evidence

- Source inspection -> the pinned QCOW2 is GPT with the Rocky root on XFS
  partition 4; its checksum remained
  `9fc9e9ff16888bb68ac39b0392e25c9c92684d50c85f1cce6ab549363bbc4b48`.
- `make image-prepare` -> converted the verified source to temporary sparse raw
  form, mounted partition 4 read-only through the pinned helper with
  `--network none`, and produced a 793,917,440-byte rootfs tar plus a
  6,445,041-byte content manifest.
- `make test-image` -> found Rocky 10.2 identity, machine-id input, x86-64
  systemd and dynamic loader; a forced second extraction produced the identical
  manifest SHA-256
  `49e69bc6209b9d8ed084e221924b633c3126ccaa9a2f920bb8464249a5f66721`;
  corrupt and missing source paths were rejected.
- `make clean` followed by cache checks and `make image-prepare` -> retained the
  prepared artifacts and manifest hash, removed temporary work, and reused the
  cache without extraction or network access.
- Revised `make demo-gate-2` -> reported eight individually named checks with
  source size/checksum, artifact sizes, Rocky identity, runtime inputs,
  provenance, both repeat-manifest hashes, cache/Git policy, and negative-path
  results. Extraction asserted a read-only source mount, no Ethernet interface,
  and no default route. The summary explicitly excluded Ginus execution, VFS,
  and systemd boot from Gate 2; all checks passed.
- `make test` -> formatting, lint, unit tests, Gate 1 Rocky interception
  regression, and Gate 2 offline repeatability tests all passed; the Rocky VM
  and extraction work were cleaned up.

## Decisions and limitations

- The Rocky source artifact is selected and verified by Gate 1; this gate
  selects only the reproducible domain-userspace preparation backend.
- The QCOW2 is a source and cache format, not a guest-kernel execution model.
- The backend converts QCOW2 to a temporary sparse raw disk on macOS, then uses
  Docker Desktop's LinuxKit kernel and a pinned Rocky helper container to mount
  the XFS root read-only. It does not boot the kernel contained in the source.
- The sorted path/metadata/content manifest is the deterministic equivalent;
  tar byte identity is not required across tool versions.
- Gate 2 is temporarily stacked on accepted Gate 1 commit `d061bc1` because
  pull request 1 remains open. Its intended eventual base is still `main`.
- The extraction helper requires a running Docker Desktop engine and a
  privileged container so LinuxKit can create a read-only loop mapping. The
  helper is pinned, receives no network, and is trusted preparation tooling;
  it is not part of the Ginus runtime boundary.

## Delivery strategy

- Working branch: `hdae/rocky-image-pipeline`
- Intended base: `main`
- Pull request: https://github.com/gmkurtzer/usshim/pull/2; dedicated branch
  push and PR creation authorized on 2026-11-03; temporarily stacked on pull
  request 1; never merge or close without explicit authorization
- Planned commits: domain-image preparation contract; preparation tools and
  Make targets; offline, corruption, and repeatability verification
- Actual commits: `04a1faf` (preparation contract) and `f58892f` (offline
  preparation, verification, and demo tooling), and `5f460c5` (verification
  evidence); `f46f0c9` records pull request delivery; `7e89e7a` makes human
  evidence explicit and strengthens extraction-isolation assertions

## Review policy and outcome

Technically passed on 2026-11-03. Human checkpoint remains open because the
prepared root becomes a dependency for all later compatibility evidence.
Present `make demo-gate-2`, fresh evidence, and the pull request before
acceptance.
