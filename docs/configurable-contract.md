# Self-contained userspace contract

Every Ginux instance carries its complete kernel/userspace contract in the
same TOML recipe as its image, resources, identity, and hooks. The recipe is
portable: host interface attachment and port translation are deployment state,
not contract policy.

```toml
[contract]
version = 1
catalog = 2
default = "deny"
allow = ["filesystem.*", "proc.*", "sys.*", "time.*"]
deny = ["device.mmio.direct", "device.dma.direct", "network.packet.raw"]

[contract.syscalls]
control = "selective"
default = "deny"
allow = ["read*", "write*", "open*", "close", "mmap", "futex", "sched_yield"]
deny = ["ptrace"]

[contract.filesystem]
default = "deny"
read = ["/"]
write = ["/run", "/tmp", "/var"]
create = ["/run", "/tmp", "/var"]
deny = ["/ginux-demo/read-only"]

[contract.network]
default = "deny"
inbound = ["tcp:*"]
outbound = ["tcp:*", "udp:53"]

[contract.devices]
default = "deny"
read = ["/dev/null", "/dev/zero", "/dev/random", "/dev/urandom", "/dev/console"]
write = ["/dev/null", "/dev/console"]
create = []

[contract.privileges]
default = "deny"
setuid = []
setgid = []

[contract.process]
default = "deny"
max_processes = 4096
max_threads = 16384
signals = ["*"]
ptrace = "deny"
namespace_create = []
namespace_join = []

[contract.kernel]
default = "deny"
proc_read = ["/"]
proc_write = []
sys_read = ["/"]
sys_write = []
```

Every subsection default is deliberately `deny`. Explicit deny wins over
allow. Runtime enforcement receives the compiled operation table and
deterministic digest; it never expands globs while managed code is running.

## Matching and precedence

`[contract].allow` and `[contract].deny` select canonical catalog operations.
Their component-aware selectors use `*` for one dot-separated component and
`**` recursively. Every selector must match the pinned catalog.

`[contract.syscalls]` uses shell-style `*` over syscall names. For example,
`open*` matches the cataloged `open` and `openat` syscalls. Every
syscall selector must match at least one syscall. Expansion occurs against the
declared catalog version, so a catalog update requires explicit revalidation
and cannot silently enlarge an existing normalized contract.

Filesystem, device, and virtual-kernel paths are absolute and component-aware.
A directory selector covers that directory and its descendants; `/private`
does not match `/private-other`. Invalid normalization and path escape fail
closed. Resolution is deterministic:

1. start at default deny;
2. apply broad grants;
3. apply surface-specific grants; and
4. apply explicit denies last.

The normalized digest includes every final action and its provenance.

## Syscall control

`control = "selective"` is the default. It permits direct host execution only
when the contract allows the syscall and the pinned catalog independently
classifies it as transparent and direct-safe. Denied, virtualized, inspected,
translated, or emulated operations remain Ginux-controlled.

`control = "paranoid"` sends every syscall through the Ginux userspace
dispatcher before completion. This provides the strongest observation point
at a substantial performance cost.

There is deliberately no `none` syscall-control value. Performance reports use
ordinary native Rocky execution as the no-Ginux control, but that control does
not provide an instance contract or containment boundary. The reproducible
native/selective/paranoid comparison is documented in
[performance.md](performance.md).

## Filesystem, devices, and privilege elevation

Filesystem and device sections use consistent `read`, `write`, and `create`
lists. A filesystem `deny` entry recursively blocks mutation below that path,
including link-based bypasses. Device `create = []` prevents character and
block device creation while ordinary FIFO creation remains governed by the
filesystem and syscall rules.

`setuid` and `setgid` are executable-path allow lists. Empty arrays permit no
set-ID execution. Existing set-ID bits do not grant virtual or host authority
unless the corresponding executable path is allowed.

## Synthetic kernel views

`/proc` and `/sys` are always synthetic, Ginux-owned views. The
`[contract.kernel]` rules control which virtual paths are visible or writable;
they never enable passthrough to the host trees.

## Network policy and deployment

`[contract.network]` defines what an instance may do. It does not select a host
interface or claim external ports. Those host-specific settings are repeatable
CLI flags, persisted separately from `config.toml`:

```sh
ginux import web recipe.toml \
    --network eth0:1 \
    --forward protocol=tcp,external_port=8080,instance_port=80 \
    --forward protocol=tcp,external_port=8443,instance_port=443

ginux set web \
    --network default \
    --forward protocol=tcp,external_port=9080,instance_port=80
```

Omitting `--network` selects the host default. `--network default` clears an
explicit attachment and `--forward none` clears forwarding. One command may
change the attachment and several forwards; Ginux validates the complete
candidate and commits it atomically. Invalid syntax, unavailable attachments,
or conflicting external ports leave the prior deployment unchanged.

## Operator workflow

```sh
ginux validate recipe.toml
ginux inspect recipe.toml
ginux explain recipe.toml filesystem.path.write
ginux diff strict.toml recipe.toml
ginux import rocky recipe.toml --network default
ginux set rocky --network eth0:1 \
    --forward protocol=tcp,external_port=8080,instance_port=80
```

`validate` parses and compiles without launching an instance. `inspect`
summarizes final decisions and the digest. `explain` identifies the winning
rule and limitation for one catalog operation. `diff` reports authority
changes between two complete recipes. There is no top-level `ginux contract`
command and the replaced `[userspace.*]` syntax fails closed.

## Bootstrap and host execution boundary

Distribution customization belongs in `[hooks].pre_boot`. Pre-boot commands
execute through Ginux mediation inside the instance root. Final contract
restrictions activate after hooks finish, allowing a recipe to prepare state
and then lock it down.

Instance-visible credentials are virtual Ginux state. Independently, every
native managed process sets `PR_SET_NO_NEW_PRIVS` before executing workload
code. A root-launched runtime drops managed host execution to UID/GID 65534; a
non-root runtime retains the caller's host identity. Virtual root therefore
does not imply host root.

Runtime audit records contain the instance, contract digest, canonical
operation, winning source rule, action, and result. The generated catalog
inventory is [`abi-contract-coverage.md`](abi-contract-coverage.md).
The default Rocky recipe uses `[audit] enabled = false`. `ginux start NAME
--audit` enables auditing for that run and records it in the instance's
`audit.log` without mixing audit records into the attached console. Set
`enabled = true` in the recipe to audit every start. Audit configuration
controls enablement only; the controller always owns the log destination.
Legacy `mode = "stderr"` and `mode = "disabled"` recipes remain readable and
normalize to `enabled = true` and `enabled = false`, respectively.
