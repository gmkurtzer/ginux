# Boot Authority Manifest v0

**Status:** Canonical measured envelope; unsigned.

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
object 2 with write. These parsed grants—not parallel hard-coded roots—seed the
control-plane capability table before its authority insertion path is sealed.
The bundle range in handoff v2 must exactly match the recorded size and digest;
only those verified bytes are copied into the ring-3 executable mapping.

The kernel hashes the exact canonical bytes and requires equality with the
digest in boot handoff v2. Sequence rollback storage, signature algorithm and
key identifiers, signature bytes, additional bundle records, platform
constraints, and independent site policy are required in later versions before
M1 can claim an authenticated immutable boot.
