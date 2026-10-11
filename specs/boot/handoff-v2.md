# Boot Handoff Contract v2

**Status:** Experimental; internal to the unified EFI image.

Version 2 retains the v1 header, UEFI memory map, ACPI RSDP, and shutdown entry,
and adds an immutable manifest byte range plus its expected SHA-256 digest. The
kernel bounds the complete range inside its transitional identity map before
retaining it or changing CR3.

The manifest and digest are currently embedded in the unified EFI image. This
means a future valid signature over that image can authenticate both values as
one unit. Until signature verification is implemented, the digest detects
corruption and establishes measurement semantics but does not authenticate an
attacker-modified image.
