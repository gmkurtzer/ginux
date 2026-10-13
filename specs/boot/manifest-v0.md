# Boot Authority Manifest v0

**Status:** Canonical development-signed envelope.

All integers are little-endian. The canonical byte layout is:

| Offset | Size | Field |
|---:|---:|---|
| 0 | 8 | `GINXMNF\0` magic |
| 8 | 2 | format version (`4`) |
| 10 | 2 | header size (`152`) |
| 12 | 4 | exact total byte size |
| 16 | 8 | nonzero monotonic sequence |
| 24 | 2 | authority-grant count |
| 26 | 2 | flags, currently zero |
| 28 | 4 | reserved, must be zero |
| 32 | 8 | exact control-plane bundle size |
| 40 | 32 | control-plane bundle SHA-256 |
| 72 | 8 | exact root-image size |
| 80 | 32 | root-image SHA-256 |
| 112 | 8 | exact resident-controller ELF size |
| 120 | 32 | resident-controller ELF SHA-256 |
| 152 | 16 each | `(object: u64, rights: u64)` grants |

The parser permits one through nine grants, requires the total size to equal
the header plus all records, rejects zero objects/rights, unknown right bits,
duplicate objects, trailing data, and nonzero reserved fields. The current
manifest authorizes host-control object 1 with read/write/manage and console
object 2 with read/write. Object 3 grants read/write access to the bounded bootstrap
IPC channel. These parsed grants—not parallel hard-coded roots—seed the
control-plane capability table before its authority insertion path is sealed.
Object 4 grants read/write access to the bounded bootstrap network-device
skeleton and can be delegated to an isolated native service.
Object 5 grants read-only access to the measured root. Object 6 grants
read/write access to the bounded persistent-volume reference service; its
rights can be attenuated independently for writer and reader domains.
Object 7 grants read/write authority for the discovered block device. The
kernel attenuates it into separate read-only observation and write-only control
domains. The latter's writable x86 leaf is necessarily readable even though
its capability lacks `READ`. Both receive only the common-configuration MMIO
page as device authority; neither receives interrupt or PCI-configuration
authority. The control proof separately owns one DMA frame, binds its exact
ring offsets, and activates an otherwise idle queue after the kernel arms bus
mastering. It may prepare the bounded sector-two read chain and receives one
additional page-granular mapping containing queue zero's validated notification
register. The current proof publishes one available entry, writes queue index
zero once, validates the bounded blank-sector completion, and then faults
without reusing the grant.
Object 8 grants read/write authority over the bounded ingress packet object;
the kernel attenuates it into device-producer `WRITE` and protocol-consumer
`READ` handles. Object 9 independently grants read/write authority over the
bounded egress packet object. The protocol service receives only its `WRITE`
half and a later isolated network consumer receives only its `READ` half.
Neither object implies authority over the other or over a network device.
The bootstrap bundle, resident controller, and root-image ranges in handoff v5
must exactly match their recorded sizes and digests. Only verified executable
bytes are copied into ring-3 mappings, and only the verified root image reaches
the initramfs parser.

The kernel hashes the exact canonical bytes, requires equality with the digest
in boot handoff v5, and strictly verifies its Ed25519 signature before parsing.
The pinned key is an RFC test identity, not a deployer trust root. Sequence
rollback storage, key identifiers and rotation, additional bundle records,
platform constraints, and independent site policy remain required for a
production authenticated boot.

After the kernel activates its own page tables, it requires the complete
manifest, measured bootstrap bundle, resident controller, and measured
root-image byte ranges to resolve through present, read-only, non-executable
identity leaves. It repeats this check after the
ring-3 scheduling, fault, and restart workload. Runtime code therefore cannot
mutate the source policy or payload bytes through their retained identity
aliases without first subverting the sealed paging hierarchy.

Every QEMU boot also invokes the canonical parser directly on malformed copies
of the authenticated bytes. The negative gate requires rejection of truncation,
nonzero reserved data, zero root-image or controller sizes, unknown rights, and
duplicate object grants, independent of the outer digest and signature checks.
The measurement gate also copies the bounded proof root, modifies one byte,
and requires the manifest-bound root verifier to reject it.

## Development generation

`make manifest` reads `boot/manifest-v0.conf`,
`control-plane/bootstrap.hex`, and the freestanding resident-controller ELF.
The bootstrap source carries the named control, service,
and restart entry labels inline. The tool wraps the decoded code and resolved
role-entry offsets in the canonical native
executable container, deterministically builds `build/initramfs-v0.cpio`,
hashes all complete artifacts and the manifest, signs the
manifest with the documented RFC test seed, and regenerates
`boot/efi/src/generated_boot.rs`. Normal build, check, run, and test targets
regenerate this file first. Production signing must use a separate tool and
non-repository key source after the trust-root policy is defined.
