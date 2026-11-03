# Gate 41: Bind Registry DNS Randomness and Responses

## Source

[GitHub issue #63](https://github.com/gmkurtzer/ginux/issues/63).

## Goal

Prevent offline prediction and unbound fallback acceptance of registry DNS
traffic used to select the image-pull endpoint.

## Approved scope

- Seed the smoltcp interface from the existing RDRAND-backed entropy source.
- Fail closed before network use when hardware entropy is unavailable.
- Bind fallback DNS responses to the outbound server/client addresses, ports,
  transaction ID, and a 64-bit fingerprint of the exact question.
- Accept A records only from the answer section of that bound response.
- Add deterministic parser regressions for valid and mismatched responses.

## Non-goals

- Do not introduce DNSSEC or a new resolver.
- Do not change registry TLS certificate and hostname verification.

## Completed work

- [x] Confirmed the interface seed was `unix_time ^ ipv4`.
- [x] Confirmed the fallback parser accepted any source-port-53 response with an A record.
- [x] Replaced the predictable seed with hardware entropy and fail-closed behavior.
- [x] Bound fallback responses to the exact outstanding query and answer section.
- [x] Added deterministic valid-response and mismatch regressions.

## Remaining work

- None.

## Acceptance criteria

- Registry interface randomness contains no wall-clock/IP-derived seed.
- Missing hardware entropy prevents registry network initialization.
- Wrong transaction IDs, endpoints, ports, or question fingerprints cannot satisfy fallback DNS.
- A bound A answer is accepted without changing the registry pull contract.
- `make check` and `make test` pass.

## Verification plan

- DNS binding tests -> the exact response succeeds; each mismatched trust field fails.
- Source inspection -> interface configuration consumes `tls_seed` and never time/IP.
- `make check` -> static, unit, policy, and service compilation checks pass.
- `make test` -> complete QEMU boot regression passes.

## Evidence

- Issue #63 matches `services/network/src/registry.rs`: the predictable seed and
  source-port-only fallback parser were both present on current main.
- The DNS binding unit tests accept the exact query response and reject changed
  transaction IDs, server addresses, client ports, and questions.
- `make check` passed with the 1,032-block unsafe baseline, network-service
  parser tests, and the unchanged 160-page root publication capacity.
- `make test` passed with `GINUX_KERNEL_OK` and `Boot test passed`.

## Decisions and limitations

- The existing TLS entropy requirement already makes RDRAND mandatory for a
  successful pull, so failing before interface creation does not remove a
  previously viable secure path.
- The first implementation exceeded the fixed root-image cap. The parser was
  reduced without enlarging that trust boundary; the final image fits 160 pages.
- DNSSEC remains outside this bounded off-path-spoofing remediation.

## Delivery strategy

- Working branch: `hdae/issue-63-registry-dns-entropy`
- Intended base: `main`
- Pull request: [#73](https://github.com/gmkurtzer/ginux/pull/73), with a
  non-closing reference to issue #63
- Planned commits: gate and DNS trust-boundary implementation; evidence and delivery
- Actual commits: `dc363b26` (gate, implementation, regressions, and evidence),
  followed by the delivery-state commit containing this update

## Review policy and outcome

Passed and closed under the user's authorized proactive-remediation policy.
All technical criteria passed and the dedicated branch and PR #73 exist; merge
remains subject to the separate two-pass policy.
