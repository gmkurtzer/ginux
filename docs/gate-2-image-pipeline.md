# Gate 2 Rocky instance-userspace pipeline

Gate 2 turns the checksum-pinned Rocky 10.2 GenericCloud QCOW2 from Gate 1
into an offline instance-userspace artifact. The source disk is data: Ginux does
not boot its kernel to prepare or run the instance.

## Preparation boundary

The reference macOS host uses `qemu-img` to convert the verified QCOW2 into a
temporary sparse raw disk. A pinned Rocky Linux container then runs on Docker
Desktop's LinuxKit kernel, attaches the raw disk read-only, mounts its XFS root
read-only, and writes two cache artifacts:

- an uncompressed, numeric-owner root filesystem tar archive; and
- a sorted content manifest containing filesystem metadata and regular-file
  SHA-256 values.

This helper kernel is tooling, not the Rocky kernel stored in the source image
and not a Ginux workload boundary. The helper receives no network access.
Preparation never modifies the source QCOW2.

The helper image is pinned in `config/rocky-instance-image.env`. Fetch it once,
when needed, with the explicit `make image-helper-fetch` acquisition target.
All preparation, validation, and demonstrations after source and helper
acquisition run with networking disabled.

## Cache and cleanup

Prepared artifacts live under `build/cache/instance/`, outside Git. `make clean`
removes temporary conversion state but preserves both the verified source and
prepared caches. `make image-cache-clean` is the explicit destructive target
for the prepared instance cache; it does not delete the Gate 1 source QCOW2 or
the Docker helper image.

The tar byte stream is not the reproducibility contract because tar tool
versions may encode equivalent metadata differently. The sorted content
manifest is the deterministic equivalent: repeated preparation from the same
verified source with the pinned helper must produce an identical manifest.

## Human verification

After the one-time source and helper acquisition:

```sh
make image-prepare
make test-image
make demo-gate-2
```

The demo verifies the pinned source, reports its Rocky identity, prepares it
without network access, compares two independently generated content
manifests, rejects a controlled corrupt source, and leaves the prepared cache
available for later gates.

The output reports eight named checks: pinned source identity, prepared
artifacts, Rocky userspace identity, required runtime inputs, provenance,
repeatability, cache/Git policy, and corrupt/missing-source failure paths. It
also verifies that the extraction mount is read-only and that the helper has
neither an Ethernet interface nor a default route. The final summary explicitly
states that Gate 2 does not test Ginux execution, Ginux VFS behavior, or booting
systemd under Ginux; those belong to later gates.
