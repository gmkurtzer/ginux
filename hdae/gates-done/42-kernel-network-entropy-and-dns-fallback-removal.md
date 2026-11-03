# Gate 42: Kernel Network Entropy and DNS Fallback Removal

## Source

[GitHub issue #63](https://github.com/gmkurtzer/ginux/issues/63), including the
follow-up report that the kernel interface seed was also public.

## Goal

Remove the remaining predictable smoltcp seed and eliminate the registry's
parallel unauthenticated DNS parsing path without enlarging the fixed root
image.

## Approved scope

- Seed the kernel smoltcp interface from the existing RDRAND-backed source.
- Fail kernel network initialization closed when entropy is unavailable.
- Remove the registry fallback parser so only smoltcp's outstanding DNS query
  can accept a response.
- Preserve the 160-page root publication capacity.

## Non-goals

- Do not introduce DNSSEC or a new resolver.
- Do not change registry TLS certificate or hostname verification.

## Completed work

- [x] Replaced the public kernel `ipv4 || gateway` seed with hardware entropy.
- [x] Removed the registry fallback response parser and its duplicate state.
- [x] Retained the RDRAND-backed registry interface seed from Gate 41.
- [x] Reduced the network service below its pre-fix release size.

## Acceptance criteria

- Neither registry nor kernel smoltcp uses public addressing or wall-clock data
  as its random seed.
- Registry DNS resolution has one response-acceptance path tied to the active
  smoltcp query.
- `make check` and `make test` pass on current `main`.

## Evidence

- The release network service is 495,864 bytes locally, compared with 496,168
  bytes before the issue #63 changes.
- `make check` passed, including manifest generation at the unchanged 160-page
  capacity and the 1,032-block unsafe baseline.
- `make test` passed with `GINUX_KERNEL_OK` and `Boot test passed`.

## Delivery strategy

- Working branch: `hdae/issue-63-registry-dns-entropy`
- Pull request: [#73](https://github.com/gmkurtzer/ginux/pull/73)
- The repaired exact head requires fresh GitHub checks and a later unchanged-head
  review before merge.

## Review policy and outcome

Technical acceptance passed locally. Delivery remains open under the separate
two-pass merge policy.
