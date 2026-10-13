# Blocked Auxiliary Cancellation v0

**Status:** The control plane stops a blocked stop-only auxiliary domain and a
fresh domain reclaims its wait key under QEMU/TCG.

The existing management `STOP` operation now accepts a stop-only auxiliary
domain whose scheduler thread is `STAGED`, `READY`, or `BLOCKED`. It never
cancels a running domain. A successful cancellation removes the generation-
bound scheduler thread and its wait reason, detaches the domain thread,
releases and scrubs the auxiliary address space, revokes its capabilities, and
publishes one `STOPPED` lifecycle event.

The executable proof blocks a read-only packet consumer on the next exact
sequence while a helper exits. With no runnable auxiliary thread remaining,
control returns to the kernel while the consumer and its packet wait remain
live. The persistent control-plane domain then uses its boot `MANAGE`
capability to stop the consumer. The proof requires the wait key, owned frames,
registered address space, and packet capability to be retired. A fresh
read-only consumer then claims that unchanged sequence, is woken by a
write-only producer, and consumes the exact frame.

## Limits

Cancellation is explicit and external; there is no packet-wait deadline or
self-cancellation operation. It applies only to stop-only auxiliary domains and
does not preempt a running domain. Device-owning domains still require their
device-specific quiescence and lease-retirement paths before memory release.
