# Self-contained userspace contract

Every Ginux instance can carry its complete kernel/userspace contract in the
same TOML file as its image, resources, devices, and networking. Runtime
enforcement receives only the compiled operation table and deterministic
digest; it never interprets globs while managed code is running.

```toml
[userspace.contract]
version = 1
catalog = 2
default = "deny"
read = ["fs.**", "proc.**", "sys.**", "time.**"]
write = ["fs.**", "dev.console.write"]
execute = ["syscall.execve"]
connect = ["network.**"]
listen = []
process = ["signal.**", "credential.**"]
system = ["memory.**", "cgroup.**"]
deny = ["dev.mmio.direct", "dev.dma.direct", "network.packet.raw"]
exact_allow = []
exact_deny = []
```

`default` is deliberately limited to `deny`. The access groups are grants;
`deny` always wins. `exact_allow` and `exact_deny` are an advanced escape hatch
for one canonical catalog operation and do not accept globs. No grant can
promote an operation marked denied, planned, or out of scope by the catalog.

## Selectors and resources

Selectors are expanded against the pinned catalog during configuration
validation. `*` matches exactly one dot-separated component and `**` matches
recursively. A wildcard must occupy its complete component, and every rule
must match at least one operation. `fs` aliases `filesystem`; `dev` aliases
`device`. Other roots use the catalog names directly:

- `fs`, `proc`, `sys`, `cgroup`, and `dev`;
- `network`, `signal`, `credential`, `time`, and `memory`; and
- `syscall` and `ioctl` for lower-level authority that lacks a higher-level
  resource representation.

Each catalog operation belongs to one human access group. This makes broad
rules such as `read = ["proc.**"]` understandable while retaining an explicit,
reviewable operation table beneath them. Provenance records the access group
and zero-based source-rule index that produced every decision.

## Resolution and migration

Resolution is deterministic:

1. deny every catalog operation;
2. apply `read`, `write`, `execute`, `connect`, `listen`, `process`, and
   `system` grants;
3. apply `exact_allow`;
4. apply `deny`; and
5. apply `exact_deny`.

The derived digest includes every final action and its provenance. Catalog
version changes require explicit revalidation, so a saved recipe cannot
silently acquire a newly cataloged operation.

Gate 10 `[contract]` profiles and templates remain accepted internally for
migration and regression comparison, but they cannot be mixed with
`[userspace.contract]`. New saved examples use the self-contained form. Copy a
complete example instance recipe as the starting point; Ginux does not create
or manage a separate contract object or generate detached contract fragments.

## Operator workflow

```sh
ginux instance validate /etc/ginux/examples/rocky-systemd.toml
ginux instance inspect /etc/ginux/examples/rocky-systemd.toml
ginux instance explain /etc/ginux/examples/rocky-systemd.toml filesystem.path.write
ginux instance diff strict.toml rocky-systemd.toml
```

`validate` parses and compiles the complete recipe without launching an
instance. `inspect` summarizes its final decisions and digest. `explain`
connects one operation to its winning human rule, constraint, and limitation.
`diff` reports authority changes between two complete instance files. The old
top-level `ginux contract` command is intentionally unsupported.

## Typed resource constraints and audit

Contract grants never bypass the typed instance fields that constrain network
addresses and ports, volume targets, assigned devices, virtual credentials,
or operation-specific syscall and ioctl arguments. Those checks still run
before any host effect.

Runtime audit records contain the instance, contract digest, canonical
operation, winning policy source, source-rule index, action, and result. Audit
detail remains bounded to 8,192 records with logarithmic dropped-event
reporting. The generated catalog inventory is
[`abi-contract-coverage.md`](abi-contract-coverage.md).

## Filesystem and elevation constraints

The same instance recipe can impose final workload constraints that are more
specific than broad contract grants:

```toml
[userspace.filesystem]
write_deny = ["/ginux-demo/read-only"]
setid = "deny"
device_nodes = "deny"
```

Each `write_deny` entry is an absolute, normalized path. A directory entry
covers that path and every descendant on a component boundary, so
`/ginux-demo/read-only/child` is denied while
`/ginux-demo/read-only-other` is unaffected. Ginux resolves confined symlinks
before comparing the target, preventing a writable alias from bypassing the
rule. Relative paths, `..`, duplicate separators, trailing separators, and
paths outside the instance root fail validation or enforcement closed.

`setid = "deny"` prevents managed code from adding set-user-ID or set-group-ID
mode bits. Existing set-ID bits do not confer virtual or host authority.
`device_nodes = "deny"` blocks new character and block device nodes while
ordinary FIFO creation remains available when the compiled contract permits
it. Link creation inside a denied tree is blocked, including hard links and
symbolic links. These controls default to `allow` when omitted for
compatibility with older recipes; security-focused recipes should state them
explicitly.

## Bootstrap and host execution boundary

Distribution customization belongs in `[hooks].pre_boot`, not in the host
helper. Pre-boot commands execute through Ginux mediation inside the instance
root. Final `write_deny`, set-ID, and device-node restrictions activate after
all hooks finish, allowing a recipe to prepare state and then lock it down.
The Rocky example uses this phase to configure its temporary test password,
systemd compatibility, and policy demonstration paths.

Instance-visible credentials are virtual Ginux state. Independently, every
native managed process sets `PR_SET_NO_NEW_PRIVS` before executing workload
code. When the runtime starts as host root, the managed process drops to host
UID/GID 65534; when started by a non-root caller, it retains that caller's host
UID/GID. Virtual root therefore does not imply host root. Operations such as
console ownership changes are emulated inside the instance rather than
recovering host privilege.
