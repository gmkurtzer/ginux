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

- [ ] None; gate not started.

## Remaining work

- [ ] Validate the Gate 1 source manifest as the preparation input.
- [ ] Implement reproducible preparation and domain-image tooling.
- [ ] Document artifact ownership, cleanup, and offline operation.
- [ ] Add corruption, missing-artifact, and repeatability tests.

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

- Not run.

## Decisions and limitations

- The Rocky source artifact is selected and verified by Gate 1; this gate
  selects only the reproducible domain-userspace preparation backend.
- The QCOW2 is a source and cache format, not a guest-kernel execution model.

## Delivery strategy

- Working branch: `hdae/rocky-image-pipeline`
- Intended base: `main`
- Pull request: intended; dedicated branch push and PR creation authorized on
  2026-11-03; never merge or close without explicit authorization
- Planned commits: domain-image preparation contract; preparation tools and
  Make targets; offline, corruption, and repeatability verification
- Actual commits: pending

## Review policy and outcome

Human checkpoint because the prepared root becomes a dependency for all later
compatibility evidence. Present
`make demo-gate-2`, fresh evidence, and the pull request before acceptance.
