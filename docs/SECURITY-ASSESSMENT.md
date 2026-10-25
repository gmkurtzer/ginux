# Ginux Initial Security Assessment

Date: October 24, 2026  
Code checkpoint: `d11c126a048a51fe7eb9c4105f31760add1027ea`

## Scope

This is a bounded first-pass review of the research prototype, not an independent
security audit, penetration test, formal verification, or production-readiness
assessment. It examined the boot trust path, memory and domain boundaries,
userspace copying, executable loading, OCI ingestion, registry TLS, dependency
advisories, and the automated verification gates.

An exploit-oriented red-team follow-up was attempted but could not continue in
the available Codex environment. OpenAI classified the request as subject to
additional cybersecurity safeguards and required Trusted Access identity
verification. This document therefore records only the bounded review described
above; it does not claim an adversarial security audit. A separately authorized
follow-up may revisit that work.

## What was run

- `make check` passed formatting, architecture consistency, Linux compatibility
  ledger validation, Clippy, and hosted unit tests.
- `cargo audit --no-fetch` scanned 118 locked dependencies against 1,352 RustSec
  advisories and reported two vulnerabilities.
- `make test` was run twice under QEMU/TCG. Both runs exercised the negative boot,
  memory-protection, capability, IOMMU, DMA, PCI, and initramfs checks, then failed
  deterministically at the linked Linux ELF probe invariant. The full-system gate
  therefore does not pass at this checkpoint.
- Manual review concentrated on cryptographic inputs, trust anchors, executable
  addresses, user-pointer validation, archive paths, redirects, certificate-name
  verification, capability generation, and unsafe-code concentration.

## Existing controls

The prototype already tests or enforces several useful controls:

- NX, W^X, supervisor write protection, SMEP, SMAP, UMIP, guard pages, canonical
  user addresses, and page ownership checks;
- generation-tagged capabilities, attenuation and revocation, cross-domain access
  rejection, and IOMMU-mediated DMA confinement;
- SHA-256 measurement of the controller and embedded components plus Ed25519
  manifest verification;
- HTTPS certificate-chain and hostname validation for registry access, OCI digest
  verification, architecture selection, and digest verification before layer
  publication; and
- checked archive arithmetic and rejection of parent-directory traversal in the
  embedded root and OCI filesystem paths.

These controls are meaningful, but they do not establish that the implementation
is secure.

## Findings

### 1. Predictable TLS random seed (high)

The registry service seeds `ChaCha8Rng` from the boot Unix time, repeating simple
multiples of that value across 32 bytes. The network stack seed adds only the IPv4
address. These inputs are observable or readily guessable and are not suitable for
TLS ephemeral-key generation. Registry TLS must use entropy obtained from a
cryptographically secure kernel interface or a validated hardware source and must
fail closed when sufficient entropy is unavailable.

### 2. Published development boot-signing key (production blocker)

The manifest tool signs every image with an RFC test seed stored in the repository,
and the kernel embeds the matching public key. The code and build output correctly
call this key development-only, but anyone can produce an image accepted by this
trust root. Production use would require external key provisioning, key rotation,
rollback protection, recovery policy, and a build process that never embeds a
private signing key.

### 3. Vulnerable cryptographic dependencies (medium)

RustSec reported:

- `RUSTSEC-2026-0387` in `crypto-bigint 0.5.5`, covering timing variability in
  operations intended to be constant-time; the advisory recommends `>=0.7.6`.
- `RUSTSEC-2023-0071` in `rsa 0.9.10`, the Marvin timing attack, severity 5.9;
  RustSec lists no fixed upgrade.

Both enter through `embedded-tls 0.19.0` in the registry network service. The TLS
dependency and enabled RSA path need replacement or a reviewed upgrade strategy.

### 4. No address randomization (medium hardening gap)

Linux personality processes use fixed heap, stack, mmap, vvar, and vDSO addresses.
The current checkpoint has no userspace ASLR or kernel KASLR. NX and W^X remain
valuable, but predictable layouts make memory-corruption exploitation easier.

### 5. Large unsafe and bespoke-parser assurance surface

A lexical inventory found 1,185 `unsafe` occurrences across 37 Rust files,
concentrated in address-space management, sockets, CPU/syscall entry, PCI, paging,
ACPI, and native services. Much of this is expected in a freestanding kernel, but
it requires focused invariants and review. Registry TLS also contains a custom SAN
parser to compensate for an upstream certificate-name limit. ELF, archive, HTTP,
TLS-name, and control-ABI parsers have targeted tests but no systematic fuzzing
program was found.

### 6. Full verification gate is red (release blocker)

The hosted suite passed, but the QEMU boot suite panicked at
`linked Linux ELF64 probe invariant failed` on two runs. Until this regression is
resolved, the checkpoint cannot claim a passing end-to-end verification baseline.

## Recommended order of work

1. Replace the registry TLS seed with a cryptographically secure entropy path and
   fail closed without it.
2. Restore the full QEMU verification gate and make it mandatory for release
   artifacts.
3. Replace the repository signing seed with externally provisioned development and
   release keys, with rollback and recovery policy.
4. Remove or upgrade the vulnerable TLS dependency chain and minimize bespoke
   certificate parsing.
5. Add fuzzing for ELF, initramfs/OCI archives, HTTP/TLS metadata, filesystem paths,
   and native control messages.
6. Add process ASLR, kernel-layout randomization where practical, stack-protection
   review, reproducible SBOM/advisory checks, and independent adversarial testing.

The result supports the paper's central caution: generating a working kernel and
establishing justified trust in that kernel are very different amounts of work.
