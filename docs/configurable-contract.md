# Configurable domain contract

Ginux resolves each domain's versioned contract before managed code starts.
Runtime enforcement receives only the resulting explicit operation table and
its deterministic digest; it never evaluates templates or wildcards while a
domain is running.

## Resolution order

1. Catalog-version default deny.
2. One versioned base profile.
3. Ordered versioned templates.
4. Domain `allow` selectors.
5. Domain `deny` selectors.
6. Validated command-line overrides, followed by complete re-resolution.

Later layers win. Within one layer, selectors are applied in listed order.
Exact selectors match one canonical operation; a selector may contain one `*`
wildcard and must match at least one operation. Domain denies are deliberately
final, so an allow cannot undo a domain deny. Unknown profiles or templates,
unversioned names, malformed or empty selectors, unmatched selectors, and a
supplied digest that differs from the derived digest fail before startup.

Profiles and templates are pinned to catalog version 1. Therefore an old
selector cannot automatically grant an operation added to a later catalog:
the configuration must be resolved and accepted against that newer catalog.

## Profiles and examples

- `strict-v1` exposes only the small identity, output, and exit surface needed
  by a negative-test workload.
- `minimal-v1` is the deny-oriented development base.
- `gate5-balanced-v1` preserves the accepted balanced native-shell surface.
- `gate9-systemd-v1` is the tested Rocky systemd baseline.
- `network-client-v1`, `read-only-time-v1`, and
  `virtual-kernel-views-v1` are additive templates.

```toml
[contract]
profile = "minimal-v1"
templates = ["read-only-time-v1"]
allow = ["network.inet.tcp.connect"]
deny = ["syscall.clock_settime"]
```

`ginux contract-plan CONFIG` prints the catalog version, digest, and every
explicit rule with its final action and provenance. The normalized domain
configuration stores the derived digest; operators do not set it through a
command-line override.

## Resource constraints

Canonical operation selectors choose authority families. Typed domain fields
constrain resources within those families:

- network rules use validated direction, protocol, CIDR/address, and bounded
  port ranges;
- port forwarding uses explicit protocol and internal/external ports;
- volume assignments use component-aware absolute domain paths and validated
  host sources;
- device assignments name registry-owned virtual devices;
- syscall and ioctl implementations validate their operation-specific
  arguments before any host effect.

These typed fields are intentionally not arbitrary contract strings. Invalid
CIDRs, ranges, paths, duplicate assignments, or conflicting forwards fail
normalization.

## Audit bounds

`audit.mode = "stderr"` emits structured native decision records containing
the domain, contract digest, canonical operation, provenance, action, and
result. A domain emits at most 8,192 detailed native audit records. Further
records are dropped without buffering; dropped counts are reported at powers
of two, bounding summary growth to at most 64 lines over a `u64` lifetime.
`audit.mode = "disabled"` suppresses these records. Audit output never grants
authority and does not include arbitrary managed-memory contents.

The generated operation inventory is
[`abi-contract-coverage.md`](abi-contract-coverage.md).
