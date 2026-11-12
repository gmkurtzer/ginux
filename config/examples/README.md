# Instance recipe examples

These recipes are complete, self-contained starting points. Copy one, change
its instance identity and policy, then validate it before importing it:

```sh
cp config/examples/agent-workspace.toml my-agent.toml
ginux validate my-agent.toml
ginux inspect my-agent.toml
ginux import my-agent my-agent.toml
```

| Recipe | Purpose | Important tradeoff |
| --- | --- | --- |
| [`../instance-rocky-systemd.toml`](../instance-rocky-systemd.toml) | Minimal Rocky systemd environment with an interactive login | Broad compatibility policy for the reference systemd workflow |
| [`agent-workspace.toml`](agent-workspace.toml) | Rocky systemd environment for an automation or coding workspace | Writes are limited to systemd runtime paths, `/workspace`, and `/tmp`; inbound networking is denied |
| [`network-service.toml`](network-service.toml) | Rocky systemd environment for a service listening on instance TCP port 8080 | The service binary is illustrative and must be supplied by the chosen image or overlay |
| [`paranoid-shell.toml`](paranoid-shell.toml) | Rocky systemd environment with every syscall mediated in userspace | Strongest syscall visibility, with substantial syscall-heavy performance cost |

Every shipped example sets the development root password to `ginux` in a
pre-boot hook. Each occurrence is marked `CHANGEME`; replace it before using a
recipe outside a local test environment. Direct-shell and service recipes do
not normally present a login prompt, but retain the same explicit development
credential for consistency.

All four recipes boot `/sbin/init` as systemd with `multi-user.target`; none
lands directly in Bash. Interactive recipes reach a console login, while the
service recipe starts `ginux-example.service` when
`/opt/service/bin/server` is present and executable.

The network-service recipe contains only portable network policy. Select the
host attachment and external forwarding when importing or changing an instance:

```sh
ginux import web config/examples/network-service.toml \
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
