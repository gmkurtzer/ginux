# Gate 4 writable-state ownership model

The generated inventory and reviewed manifest classify every mutable Rust
static in the boot, kernel, control-plane, and service source trees. The
manifest key is `(kind, file, symbol)`; source line numbers are deliberately not
part of the key so ordinary edits do not invalidate it.

## Ownership groups

| Group | Owner and lifetime | Access and synchronization invariant | Disposition |
|---|---|---|---|
| `bootstrap-init` | Firmware/boot CPU until `ExitBootServices` or immutable handoff | One writer before kernel concurrency; no runtime mutation | Narrow allowlist |
| `per-cpu-arch` | One logical CPU for that CPU's online lifetime | Only the owning CPU mutates local entry state; cross-CPU control requires the Gate 8 synchronization protocol | Gate 6 state container, activated by Gate 9 |
| `domain-runtime` | One domain generation from creation through teardown | Access requires the matching generation; shared management uses the Gate 8 lock order | Gate 5 migration |
| `process-runtime` | One Linux process or thread generation | Current task or scheduler-owned transition may mutate it; publication and teardown are synchronized in Gate 5/8 | Gate 5 migration |
| `shared-runtime` | Kernel runtime manager for the boot lifetime | Explicit manager API; no implicit protection from local interrupt masking | Gate 5 migration |
| `platform-device` | Host platform or device-generation owner | Driver/service API owns transitions; interrupt and process contexts use Gate 8 synchronization | Gate 6 migration |
| `service-runtime` | One isolated native-service generation | Service-local serialization plus generation-checked kernel publication | Gate 6 migration |
| `verification-only` | Verification boot harness for one bounded test | Never authoritative runtime state; reset before and after the test | Narrow allowlist |

## Lifecycle and lock-order requirements

Gate 5 creates explicit runtime, domain, process, and thread state containers
with initialization, generation validation, reset, and destruction paths. Gate
6 does the same for per-CPU architecture anchors, platform devices, storage,
networking, and isolated services. Gate 8 supplies synchronization only after
those owners are concrete.

The required acquisition direction is:

```text
platform/CPU registry -> runtime manager -> domain -> process -> thread -> descriptor/object
```

Interrupt handlers may acquire only interrupt-safe locks and may not reverse
that order. Local interrupt masking protects only CPU-local state and is never
treated as mutual exclusion for a shared owner.

## Enforcement

`writable-globals.tsv` is the reviewed classification manifest.
`check-writable-globals.sh` regenerates the inventory, requires an exact
one-to-one manifest match, validates every group/disposition/gate, rejects
duplicates, and runs a negative test proving that a synthetic unclassified
global changes the checked set. New writable statics therefore fail
`make architecture-check` until they receive an explicit disposition.
