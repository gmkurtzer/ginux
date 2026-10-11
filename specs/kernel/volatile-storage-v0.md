# Volatile Storage Service v0

**Status:** Single-slot, register-backed protocol verified from ring 3.

The restarted supervised service implements the first bounded storage-shaped
userspace workload. It receives value 3, retains that word in its private CPU
context, and returns acknowledgement 4. It then blocks for fixed GET command 5
and returns the retained value 3. The control plane validates both replies and
the kernel audits the final request and reply as sequence three from the exact
replacement generation.

This is an execution and isolation proof, not a filesystem or stable storage
ABI. The value is lost when the service exits or faults. There is one fixed
value, one fixed command, one client, no names, no request identifiers, no
concurrency, no recovery, and no persistence across service restart or machine
reboot. A later storage protocol must define those semantics before exposing a
general-purpose interface.
