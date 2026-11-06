# Gate 9.75 domain specification

Ginux uses one strict, versioned TOML document for the complete initial domain
definition. Values are applied in this order:

1. built-in defaults;
2. the configuration file; and
3. typed `--set KEY=TOML_VALUE` command-line overrides.

The result is normalized, validated, and saved as the domain's immutable
effective configuration before any managed process starts. Unknown fields,
invalid combinations, unavailable network attachments, conflicting translated
ports, protected virtual-tree volume mappings, and host-context hooks fail
before startup.

## Configuration classes

| Section | Purpose |
|---|---|
| `domain`, `identity` | Domain ID, hostname, image, init, arguments, environment, and work directory |
| `resources` | Virtual CPU and memory assignment |
| `execution` | Explicit runtime backend and security/performance profile |
| `contract` | Base contract, additional templates, allow/deny selectors, and expanded-contract digest |
| `storage`, `volumes`, `devices` | Domain storage, reviewed host mappings, and assigned device frontends |
| `network` | Backend, host interface/source, ingress/egress policy, and translated ports |
| `access` | Initial virtual UID and GID |
| `hooks` | Ordered domain-context pre-boot commands, timeout, and failure policy |
| `lifecycle`, `audit`, `failure` | Stop bound, audit mode, and runtime failure behavior |

[`config/domain-gate9.75.toml`](../config/domain-gate9.75.toml) is the complete
schema example. Gate 10 owns expansion and enforcement of the contract
selectors; Gate 9.75 records the selected inputs and immutable digest.

## Effective configuration and overrides

Create a domain from a file and override only the named values:

```sh
ginux domain create rocky --config /etc/ginux/rocky.toml \
    --set resources.vcpus=4 \
    --set 'identity.hostname="build.example"' \
    --set 'network.interface="eth0"'
```

The saved `/var/lib/ginux/domains/rocky/config.toml` is the exact normalized
configuration used by startup. Unspecified file values and defaults remain
unchanged. `ginux config-merge FILE KEY=TOML_VALUE...` prints the same
normalized result without creating a domain.

## Shared networking and translated ports

If the `network` section is omitted, the domain uses the host-shared backend.
An explicit interface or source address selects the host attachment used by
mediated sockets. The address must exist on that interface when the domain is
created; if only an interface is supplied, Ginux records its resolved IPv4
source in the effective configuration.

Rules use `tcp:PORT`, `udp:PORT`, or a `*` port. Deny rules take precedence;
an empty allow list means that otherwise-undisallowed ports remain available.
A translation uses `protocol/attachment/external/internal`, for example:

```toml
[network]
backend = "shared"
interface = "eth0:1"
source = "192.0.2.10"
ingress_allow = ["tcp:80"]
egress_deny = ["tcp:25"]
port_forwards = ["tcp/eth0:1/8080/80"]
```

The domain binds and observes port 80 while Ginux owns host port 8080 on the
selected attachment. Untranslated binds retain ordinary host conflict
semantics. A translated host port or domain port may appear only once per
protocol, preventing ambiguous ownership.

## Pre-boot hooks

Pre-boot hooks run in listed order after declarative root/resource preparation
and before init. Each hook is a managed `/bin/sh -c` process inside the domain
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
