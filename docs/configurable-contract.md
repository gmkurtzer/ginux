# Self-contained userspace contract

Every Ginux instance can carry its complete kernel/userspace contract in the
same TOML file as its image, resources, devices, and networking. Runtime
enforcement receives only the compiled operation table and deterministic
digest; it never interprets globs while managed code is running.

```toml
[userspace.contract]
version = 1
catalog = 1
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

Gate 10 `[contract]` profiles and templates remain accepted for migration and
regression comparison, but they cannot be mixed with `[userspace.contract]`.
New saved examples use the self-contained form. `ginux contract init
systemd-v1` and `ginux contract init strict-v1` print complete recipe sections;
the generated text has no runtime preset dependency.

## Operator workflow

```sh
ginux contract init systemd-v1
ginux contract check /etc/ginux/examples/rocky-systemd.toml
ginux contract show /etc/ginux/examples/rocky-systemd.toml
ginux contract inspect /etc/ginux/examples/rocky-systemd.toml
ginux contract explain /etc/ginux/examples/rocky-systemd.toml filesystem.path.write
ginux contract diff strict.toml rocky-systemd.toml
```

`check` validates and compiles without launching an instance. `show` prints the
saved recipe and derived digest. `inspect` summarizes final decisions.
`explain` connects one operation to its winning human rule, constraint, and
limitation. `diff` reports authority changes between two complete instance
files.

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
