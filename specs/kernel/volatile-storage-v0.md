# Volatile Storage Service v0

**Status:** Single-slot, register-backed protocol verified from ring 3.

The restarted supervised service implements the first bounded storage-shaped
userspace workload. It receives value 3, retains that word in its private CPU
context, and returns acknowledgement 4. It then blocks for fixed GET command 5
and returns the retained value 3. The control plane validates both replies and
the storage round trip occupies request and reply sequence three for the exact
replacement generation.

After returning the value, the service exits normally. Supervision publishes a
kind 7 clean-exit record for that exact generation, distinct from the original
generation's kind 1 fault, and the control plane validates its structured
descriptor.

Command 6 is deliberately unsupported. The service remains isolated and
responsive, returns `UINT64_MAX - 2`, and then exits cleanly. The control plane
requires that exact error and audits the fourth request/reply sequence, proving
that malformed protocol input is rejected without a service fault or hang.

This is an execution and isolation proof, not a filesystem or stable storage
ABI. The value is lost when the service exits or faults. There is one fixed
value, one fixed command, one client, no names, no request identifiers, no
concurrency, no recovery, and no persistence across service restart or machine
reboot. The separate [Persistent Volume Record v0](persistent-volume-v0.md)
defines a crash-consistent two-slot block format and recovery algorithm.
Control ABI v4 now connects that format to a distinct capability-gated
[Persistent Volume Service v0](volume-service-v0.md), but does not retrofit
durability into this older IPC protocol or provide a real block backend.
