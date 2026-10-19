# Host Storage

Ginux uses one host-owned block volume for durable system objects. Under QEMU,
`make run` creates `build/qemu/volume.raw` as a sparse 64 GiB file. The volume
size is not a runtime allocation cap: sparse backing storage grows as sectors
are written, and workload memory remains independently configured per domain.

## On-disk foundation

The first eight 512-byte sectors are reserved for host metadata. Sectors 0 and
1 hold the two transactional record slots. A committed record contains a
versioned host-volume header with the exact device geometry and the start and
length of the data region. Data begins at sector 8.

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

The durable data region is currently reserved but unused. OCI content-addressed
objects, assembled image metadata, domain/system configuration, writable
layers, and logs require separate transactional schemas and lifecycle policy;
they will be added in bounded review checkpoints rather than coupled to the
superblock format.
