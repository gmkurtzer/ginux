# Gate 33: Pinned Top-Level Manifest Digest

## Source

[GitHub issue #16](https://github.com/gmkurtzer/ginux/issues/16).

## Goal

Make an OCI image reference using `@sha256:<digest>` cryptographically bind
the exact top-level registry manifest or index body before Ginux parses it or
follows any descriptor.

## Approved scope

- Verify the fetched top-level response body against a digest selector.
- Reject a mismatch before index selection or manifest parsing.
- Preserve tag-selected pulls without imposing a digest comparison.
- Add deterministic positive, mismatch, and tag-path regression tests.

## Non-goals

- Do not redesign registry transport, authentication, TLS, or OCI descriptor
  processing.
- Do not change sub-manifest, configuration, or layer verification semantics.
- Do not merge the pull request or close issue #16.

## Completed work

- [x] Confirmed the digest selector currently affects only the request path.
- [x] Confirmed selected manifests, configurations, and layers are verified,
  but the initially fetched top-level body is not.
- [x] Added host-tested conditional digest-pin verification to the OCI
  reference boundary.
- [x] Enforced the pin immediately after top-level transfer and before index
  selection or manifest parsing.

## Remaining work

- [x] Add reusable digest-selector verification and deterministic tests.
- [x] Enforce it on the top-level body before parsing.
- [ ] Run targeted, static, and complete QEMU verification.
- [ ] Commit and push the dedicated branch.
- [ ] Open the remediation pull request and add reciprocal source links.

## Acceptance criteria

- A digest-pinned reference accepts the exact body matching its SHA-256 pin.
- A digest-pinned reference rejects any other body before descriptor use.
- A tag-selected reference retains existing behavior.
- Existing selected-manifest, configuration, layer, build, and boot checks pass.

## Verification plan

- `cargo test --package ginux-oci` -> exact pin passes, mismatch fails, and tag
  selection remains unaffected.
- Source inspection -> top-level verification runs immediately after transfer
  and before index selection or manifest parsing.
- `make check` -> formatting, architecture, lint, and unit checks pass.
- `make test` -> complete QEMU verification emits `GINUX_KERNEL_OK` and
  `Boot test passed.`

## Evidence

- `ImageReference::write_manifest_path` includes the selector in the URL.
- The top-level body currently reaches `select_manifest` or
  `parse_registry_manifest` without comparison to `reference.selector`.
- `cargo test --package ginux-oci` -> 9 tests passed, including exact pinned
  body acceptance, changed-body rejection, and unchanged tag behavior.
- Warning-denying clippy for `ginux-oci` and the freestanding network service
  -> passed.

## Decisions and limitations

- The reference parser already validates the `sha256:` selector shape; the
  OCI library will own the conditional body comparison so it is host-testable.
- The pin binds the response bytes exactly, as required by OCI digest
  semantics; registry reserialization under the same digest path is rejected.

## Delivery strategy

- Working branch: `hdae/issue-16-manifest-digest`
- Intended base: `main`
- Pull request: intended; non-closing reference to issue #16
- Planned commits: gate and regression mechanism; integration and evidence;
  delivery checkpoint
- Actual commits: `bb9b7d16` gate definition; implementation and verification
  evidence pending

## Review policy and outcome

Delegated by the user's request to resolve issue #16 next. Keep the gate in
`gates-todo/` at the pull-request checkpoint for maintainer review.
