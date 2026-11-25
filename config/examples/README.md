# Instance recipe examples

These recipes are complete, self-contained starting points. Copy one, change
its instance identity and policy, then validate it before importing it:

```sh
cp config/examples/rocky10-agent-workspace.toml my-agent.toml
ginux validate my-agent.toml
ginux inspect my-agent.toml
ginux import my-agent my-agent.toml
```

| Recipe | Purpose | Important tradeoff |
| --- | --- | --- |
| [`rocky10-systemd.toml`](rocky10-systemd.toml) | Minimal Rocky systemd environment with an interactive login | Broad compatibility policy for the reference systemd workflow |
| [`rocky10-agent-workspace.toml`](rocky10-agent-workspace.toml) | Rocky systemd environment for an automation or coding workspace | Writes are limited to systemd runtime paths, `/workspace`, and `/tmp`; inbound networking is denied |
| [`ubuntu24-network-service.toml`](ubuntu24-network-service.toml) | Ubuntu 24.04 systemd environment serving a blank CMS on instance TCP port 8080 | Uses the digest-pinned third-party `jrei/systemd-ubuntu` OCI root and a small Python HTTP service |
| [`rocky10-paranoid-shell.toml`](rocky10-paranoid-shell.toml) | Rocky systemd environment with every syscall mediated in userspace | Strongest syscall visibility, with substantial syscall-heavy performance cost |
| [`alpine3.22-native.toml`](alpine3.22-native.toml) | Minimal Alpine 3.22 environment using its native BusyBox init | Smallest example; intentionally does not add or emulate systemd |

Every shipped example sets the development root password to `ginux` in a
pre-boot hook. Each occurrence is marked `CHANGEME`; replace it before using a
recipe outside a local test environment. Direct-shell and service recipes do
not normally present a login prompt, but retain the same explicit development
credential for consistency.

Every recipe boots the distribution's native init path and none lands directly
in Bash. Rocky and Ubuntu use systemd; Alpine uses BusyBox init. Interactive
recipes reach a console login, while the Ubuntu service recipe starts the
included `ginux-cms.service` and serves its blank page on port 8080.

The Ubuntu CMS recipe uses the third-party `jrei/systemd-ubuntu` image because
the official minimal Ubuntu OCI image does not ship `/sbin/init`. Its exact
manifest is digest-pinned and verified by the same generic OCI acquisition path
as the other examples.

The Ubuntu CMS recipe contains only portable network policy. Select the host
attachment and external forwarding when importing or changing an instance:

```sh
ginux import web config/examples/ubuntu24-network-service.toml \
  --network default \
  --forward protocol=tcp,external_port=8080,instance_port=8080
```

## Readable pre-boot scripts

`[hooks].pre_boot` is an ordered array. A short command can remain a regular
quoted string; related commands can use a triple-quoted multiline string:

```toml
[hooks]
pre_boot = [
  "/usr/bin/cp /etc/example.default /etc/example",
  """
  /usr/bin/mkdir -p /workspace
  /usr/bin/chown 1000:1000 /workspace
  """,
]
timeout_seconds = 10
failure = "stop"
```

Each array item is still one hook. Ginux runs it through `/bin/sh -c` inside
the instance root with the existing timeout, failure policy, mediation, and
audit behavior. A multiline string is embedded recipe content, not a path to a
host script. The first executable must remain an absolute instance path.

Hooks are intentionally bounded to 12 items and 512 bytes per item. Invalid,
oversized, unterminated, relative, or host-context hooks fail validation.
