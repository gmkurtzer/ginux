# Bootstrap Cryptography v0

**Status:** SHA-256 measurement plus strict Ed25519 verification.

The privileged boot path contains a small allocation-free SHA-256
implementation so manifest and bundle integrity checks do not depend on a
runtime crypto service. It accepts an arbitrary byte slice, processes full and
padded blocks with checked slice boundaries, and returns the 32-byte digest.

Every QEMU boot executes the standard SHA-256 known-answer tests for the empty
message and `abc`. Failure stops boot before the success marker.

Manifest authentication uses the pinned, no-std `ed25519-dalek` 3.0.0 verifier
with strict verification. Each boot verifies the valid manifest signature and
proves a one-bit-corrupted signature is rejected. The current public key is the
RFC 8032 test identity and is explicitly development-only; its corresponding
published test seed is not a production secret.

M1 still requires production key provisioning and rotation, signer separation,
rollback semantics, and signed policy for every boot-authorized native artifact.
The runtime crypto service remains separate and replaceable; this bootstrap
boundary must stay narrow and independently reviewable.
