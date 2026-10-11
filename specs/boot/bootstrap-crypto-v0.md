# Bootstrap Cryptography v0

**Status:** SHA-256 digest primitive only; no signature verification yet.

The privileged boot path contains a small allocation-free SHA-256
implementation so manifest and bundle integrity checks do not depend on a
runtime crypto service. It accepts an arbitrary byte slice, processes full and
padded blocks with checked slice boundaries, and returns the 32-byte digest.

Every QEMU boot executes the standard SHA-256 known-answer tests for the empty
message and `abc`. Failure stops boot before the success marker.

This primitive provides hashing, not authenticity. M1 still requires a
canonical manifest format, a signed envelope, pinned public-key policy,
rollback semantics, and verification of every boot-authorized native artifact.
The runtime crypto service remains separate and replaceable; this bootstrap
implementation must stay narrow and independently reviewable.
