# Boot-local image store v0

The C3 OCI cache is volatile, available-memory-backed, and content addressed.
It has no configured byte ceiling and exposes no partially downloaded object
to a container.

An ingestion client reserves the complete declared object length before the
first byte is accepted. The reservation succeeds while backing memory is
available; resource exhaustion is not an image-size policy. Bytes are appended
in order while SHA-256 is computed.
Publication succeeds only when the declared length and digest both match. A
failed transfer, digest mismatch, or explicit abort destroys and scrubs its
staging storage before returning its backing memory.

Published objects are immutable and addressed by their SHA-256 digest. Staging
handles include a monotonically increasing generation so an aborted handle
cannot mutate a later object that reuses its metadata slot.

The initial QEMU implementation uses RAM-backed pages. Every object and all
container configuration disappear when the host reboots. Persistent storage is
a later checkpoint and must preserve the same verify-before-publish rule.
