# Boot Handoff Contract v2

**Status:** Superseded by `handoff-v3.md`.

Version 2 retains the v1 header, UEFI memory map, ACPI RSDP, and shutdown entry,
and adds immutable manifest and control-plane-bundle byte ranges plus the
manifest's expected SHA-256 digest. The kernel bounds both complete ranges
inside its transitional identity map before retaining them or changing CR3.

The manifest, digest, and Ed25519 signature are currently embedded in the
unified EFI image. The kernel strictly verifies the signature against a pinned
development public key before parsing authority. Production must replace the
RFC test identity and define key rotation, revocation, and rollback policy.
