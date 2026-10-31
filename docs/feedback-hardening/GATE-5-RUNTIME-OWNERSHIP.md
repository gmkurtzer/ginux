# Gate 5 Runtime Ownership Evidence

Gate 5 replaces ambient process, domain, scheduler, signal, syscall, and x86-64
execution state with explicit owner roots. It does not add the cross-CPU
synchronization required for SMP; that remains Gate 8.

## Result

- All Gate 4 entries classified for Gate 5 have been migrated.
- The x86-64 execution bank is reachable through one `X86RuntimeOwner` root.
  Its 403 stable backing objects are private implementation storage rather than
  ambient lookup points.
- Five raw mutable x86-64 runtime banks are owned by
  `X86MutableRuntimeOwner`; their existing single-CPU access semantics are
  deliberately unchanged until Gate 8.
- Domain, process, socket, signal, keyring, AIO, namespace, tmpfs, SysV IPC,
  overlay, file-cache, and suspended-runtime state use explicit lifecycle
  owners established by the incremental Gate 5 commits.
- The writable-global inventory contains no row assigned to migration Gate 5.

## Enforcement

`tools/architecture/check-x86-runtime-owner.sh` verifies that every x86-64
runtime backing object is initialized exactly once by the owner and rejects a
direct access outside the declaration and owner initializer. Its negative test
injects such a bypass into a temporary copy and requires the check to fail.

The normal architecture check also inventories every writable global and
requires an ownership contract for each allowlisted object.

## Lifecycle and isolation evidence

The verification boot exercises generation-bound domain identity, cross-domain
capability revocation, process and scheduler ownership, auxiliary runtime
teardown, domain retirement, service restart, Linux return ownership, and
cleanup of queued, ready, and yielded auxiliary work. The Linux ABI controller
suite separately checks 23 interfaces and 375 syscall contracts.

The detached controller path also now yields to queued domain lifecycle work
when the controller is blocked waiting for input. This prevents the controller
read from starving background domain startup.

## External integration limitation

`make test-multi-domain` currently advances detached lifecycle work but then
stalls when a floating upstream Alpine image enters Linux userspace. The same
failure reproduces on pre-Gate-5 commits `0095ff55`, `900d0d56`, and
`c345a0d3`, so it is not evidence of a Gate 5 ownership regression. The test
was interrupted and its cleanup trap removed QEMU. The deterministic ABI and
verification-boot suites are the Gate 5 regression evidence; compatibility
with the changed external Alpine artifact is separate follow-up work.

## Commands

- `make architecture-check`
- `make check`
- `make test-linux-abi`
- `make build-verification && bash tests/qemu/boot.sh`
- `make test-multi-domain` (external-image limitation above)
