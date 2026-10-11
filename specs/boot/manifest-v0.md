# Boot Authority Manifest v0

**Status:** Canonical development-signed envelope.

All integers are little-endian. The canonical byte layout is:

| Offset | Size | Field |
|---:|---:|---|
| 0 | 8 | `GINXMNF\0` magic |
| 8 | 2 | format version (`2`) |
| 10 | 2 | header size (`72`) |
| 12 | 4 | exact total byte size |
| 16 | 8 | nonzero monotonic sequence |
| 24 | 2 | authority-grant count |
| 26 | 2 | flags, currently zero |
| 28 | 4 | reserved, must be zero |
| 32 | 8 | exact control-plane bundle size |
| 40 | 32 | control-plane bundle SHA-256 |
| 72 | 16 each | `(object: u64, rights: u64)` grants |

The parser permits one through eight grants, requires the total size to equal
the header plus all records, rejects zero objects/rights, unknown right bits,
duplicate objects, trailing data, and nonzero reserved fields. The current
manifest authorizes host-control object 1 with read/write/manage and console
object 2 with write. Object 3 grants read/write access to the bounded bootstrap
IPC channel. These parsed grants—not parallel hard-coded roots—seed the
control-plane capability table before its authority insertion path is sealed.
Object 4 grants read-only discovery of the bounded bootstrap network-device
skeleton and can be delegated to an isolated native service.
The bundle range in handoff v3 must exactly match the recorded size and digest;
only those verified bytes are copied into the ring-3 executable mapping.

The kernel hashes the exact canonical bytes, requires equality with the digest
in boot handoff v3, and strictly verifies its Ed25519 signature before parsing.
The pinned key is an RFC test identity, not a deployer trust root. Sequence
rollback storage, key identifiers and rotation, additional bundle records,
platform constraints, and independent site policy remain required for a
production authenticated boot.

After the kernel activates its own page tables, it requires the complete
manifest and measured bundle byte ranges to resolve through present,
read-only, non-executable identity leaves. It repeats this check after the
ring-3 scheduling, fault, and restart workload. Runtime code therefore cannot
mutate the source policy or payload bytes through their retained identity
aliases without first subverting the sealed paging hierarchy.

Every QEMU boot also invokes the canonical parser directly on malformed copies
of the authenticated bytes. The negative gate requires rejection of truncation,
nonzero reserved data, unknown rights, and duplicate object grants, independent
of the outer digest and signature checks.

## Development generation

`make manifest` reads `boot/manifest-v0.conf`,
`control-plane/bootstrap.hex`. That source carries the named control, service,
and restart entry labels inline. The tool wraps the decoded code and resolved
role-entry offsets in the canonical native
executable container, hashes the complete bundle and manifest, signs the
manifest with the documented RFC test seed, and regenerates
`boot/efi/src/generated_boot.rs`. Normal build, check, run, and test targets
regenerate this file first. Production signing must use a separate tool and
non-repository key source after the trust-root policy is defined.
