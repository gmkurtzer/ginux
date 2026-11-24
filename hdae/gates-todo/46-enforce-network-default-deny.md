# Enforce network default-deny semantics

## Source

[Issue #298](https://github.com/gmkurtzer/ginux/issues/298).

## Goal

Make an empty inbound or outbound allow list deny every corresponding network
operation, matching the only supported contract default and the documented
default-deny model.

## Scope

- Remove the implicit allow-all behavior for empty network allow lists.
- Preserve explicit protocol, port, and wildcard grants.
- Add focused ingress and egress regressions for empty and explicit lists.

## Acceptance criteria

- Empty inbound and outbound lists deny TCP and UDP operations.
- Explicit exact-port and wildcard rules continue to allow only their matches.
- Existing configuration, static, unit, and network-policy tests pass.

## Non-goals

- No allow-by-default contract mode is introduced.
- No network backend or destination mediation behavior changes.

## Status

Pull request #306 is merged into `main`. Technical status remains passed;
lifecycle status remains open pending the consolidated Gate 51 human
acceptance checkpoint.

## Evidence

- `cargo test -p ginux-instance network_default_deny_rejects_empty_allow_lists`
- `cargo test -p ginux-instance`
- `make check`
- Fresh Gate 51 `make check test-cli test-unit`, `make test-contract`,
  `make test-security`, concurrent-instance networking, and aggregate
  `make test` revalidate empty-list default denial and explicit grants on
  current `main`.
