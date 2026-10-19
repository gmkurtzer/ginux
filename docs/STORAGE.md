# Host Storage

Ginux uses one host-owned block volume for durable system objects. Under QEMU,
`make run` creates `build/qemu/volume.raw` as a sparse 64 GiB file. The volume
size is not a runtime allocation cap: sparse backing storage grows as sectors
are written, and workload memory remains independently configured per domain.

## On-disk foundation

The first eight 512-byte sectors are reserved for host metadata. Sectors 0 and
1 hold the two host-volume transactional record slots. A committed record contains a
versioned host-volume header with the exact device geometry and the start and
length of the data region. Data begins at sector 8.

Local configuration uses two independent 32-sector slots at sectors 8-39 and
40-71. Each slot carries its own version, generation, length, SHA-256 digest,
and commit marker. It records domain definitions and resource settings,
virtual-network topology and attachments, and host static/DHCP network policy.
Runtime and failed/running process state is deliberately not restored; domains
return as configured objects after reboot.

The OCI cache catalog uses independent 64-sector transactional slots at
sectors 72-135 and 136-199. Cached layer extents begin at sector 256 and grow
on demand to the volume geometry; there is no fixed byte quota below the
volume size. Catalog entries bind a canonical image reference and platform to
an ordered layer list. Layers are keyed by SHA-256 digest and shared between
images when their digest and length match.

New layer bytes are written and flushed before a new catalog generation is
prepared and committed. A crash before catalog commit can leave only
unreachable sectors. Recovery never exposes them. Restoring a layer streams
it through the normal image-store digest verifier before publication, so disk
corruption fails closed and falls back to a registry pull.

A metadata update follows prepare, flush, commit, flush. Recovery accepts only
the newest unambiguous committed generation with a valid SHA-256 digest. Empty
media is initialized. The earlier development marker is migrated
transactionally. Foreign, corrupt, undersized, or geometry-mismatched media is
not silently reformatted.

## Development lifecycle

- `make run` creates the volume if it is absent and recovers it otherwise.
- `GINUX_VOLUME_SIZE=SIZE make run` selects the initial sparse size.
- `make reset-storage` deletes only the default development volume.
- The next `make run` recreates a clean volume.
- `make test-storage` proves initialization, reboot recovery, deletion,
  recreation, and process cleanup across three QEMU boots.
- `make test-oci-cache` performs a real cold registry pull, interrupts the
  host after durable commit, reboots the same volume, and proves the image is
  restored without registry authentication or layer transfer.

The remaining work is cache refresh and garbage collection, assembled-image
metadata, writable domain layers, explicit persistent volumes, and logs. Each
requires its own lifecycle policy and bounded review checkpoint.
