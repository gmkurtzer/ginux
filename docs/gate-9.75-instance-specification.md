# Gate 9.75 instance specification

Ginux uses one strict, versioned TOML document for the complete initial instance
definition. Values are applied in this order:

1. built-in defaults;
2. the configuration file; and
3. typed `--set KEY=TOML_VALUE` command-line overrides.

The portable recipe is normalized, validated, and saved before any managed
process starts. Host-specific network attachment and translated ports are
validated separately and persisted as deployment state. Unknown fields,
invalid combinations, unavailable network attachments, conflicting translated
ports, protected virtual-tree volume mappings, and host-context hooks fail
before startup.

## Configuration classes

| Section | Purpose |
|---|---|
| `instance`, `identity` | Instance ID, hostname, image, init, arguments, environment, and work directory |
| `resources` | Virtual CPU and memory assignment |
| `execution` | Explicit runtime backend and security/performance profile |
| `contract` and its subsections | Self-contained operation, syscall, filesystem, network, device, privilege, process, and synthetic-kernel policy |
| `storage`, `volumes`, `devices` | Instance storage, reviewed host mappings, and assigned device frontends |
| `network` | Instance network backend; access policy belongs to `contract.network` |
| `access` | Initial virtual UID and GID |
| `hooks` | Ordered instance-context pre-boot commands, timeout, and failure policy |
| `lifecycle`, `audit`, `failure` | Stop bound, audit mode, and runtime failure behavior |

[`config/instance-gate9.75.toml`](../config/instance-gate9.75.toml) is the complete
schema example. Gate 10.5 owns the self-contained contract recipe and its
explanation workflow; Gate 9.75 records the remaining instance inputs.

## Effective configuration and overrides

Create an instance from a file and override only the named values:

```sh
ginux instance create rocky /etc/ginux/rocky.toml \
    --set resources.vcpus=4 \
    --set 'identity.hostname="build.example"' \
    --network eth0 \
    --forward protocol=tcp,external_port=8080,instance_port=80
```

The saved `/var/lib/ginux/instances/rocky/config.toml` is the normalized
portable recipe. `/var/lib/ginux/instances/rocky/deployment.state` stores the
host-specific network settings, and `runtime.toml` is materialized from both
before startup. Multiple flags are validated and committed atomically.
Unspecified file values and defaults remain unchanged. `ginux config-merge
FILE KEY=TOML_VALUE...` prints a normalized recipe without creating an
instance.

## Shared networking and translated ports

If `--network` is omitted, the instance uses the host's default network. An
explicit attachment selects the host interface or alias used by mediated
sockets. It must exist when the instance is created or changed.

Contract rules use `tcp:PORT`, `udp:PORT`, or a `*` port. A translation names
both sides explicitly:

```sh
ginux instance set rocky \
    --network eth0:1 \
    --forward protocol=tcp,external_port=8080,instance_port=80
```

The instance binds and observes port 80 while Ginux owns host port 8080 on the
selected attachment. Untranslated binds retain ordinary host conflict
semantics. A translated external port or instance port may appear only once
per protocol, preventing ambiguous ownership. `--network default` clears an
explicit attachment; `--forward none` clears all translations.

## Pre-boot hooks

Pre-boot hooks run in listed order after declarative root/resource preparation
and before init. Each hook is a managed `/bin/sh -c` process inside the instance
root and uses the same ptrace mediation, contract, virtual identity, devices,
and network policy as init. It receives no host path, descriptor, credential,
or helper authority. Host-context hooks such as `host:/path` are rejected.

```toml
[hooks]
pre_boot = [
  "/usr/bin/install -d /run/example",
  "/usr/bin/printf ready > /run/example/state",
]
timeout_seconds = 20
failure = "stop"
```

`failure = "stop"` prevents init from running after a failure or timeout.
`failure = "continue"` records the failure and advances to the next hook. Every
dispatch and result is audited with its index, context, timeout, and status.

## Verification

```sh
make test-config
make test-gate975-network
make test-gate975-hooks
make demo-gate-9.75
```

The QEMU checks cover typed precedence, invalid configuration, attachment
resolution, ingress/egress denial, translated-port delivery, conflicts,
cleanup, hook ordering, denied authority, failure policy, and timeout.
