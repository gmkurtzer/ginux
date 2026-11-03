# fuzz/ — libFuzzer harnesses for the untrusted-input parser surfaces

A standalone cargo-fuzz workspace covering the three crates that parse bytes
derived from untrusted input. This directory is intentionally **not** a member
of the root workspace: upstream crates keep resolving `license.workspace =
true` against the root manifest, and no root file is modified.

## Targets

| target | crate | surfaces | seeds | dictionary |
|---|---|---|---|---|
| `oci_surfaces` | `crates/oci` | tar/PAX rootfs parse + traversal mirroring the kernel published-rootfs call pattern, registry manifest, manifest index selection, image configuration, image reference, digest validation, bearer challenge/token, sha256 verification | `corpus-seed/` (53) | `oci.dict` |
| `protocol_surfaces` | `crates/protocol-state` | ARP reply, IPv4+ICMP parse, ICMP echo request/reply builders, neighbor state machine (kernel-faithful fixed endpoints, 65,600-byte cap) | `corpus-seed-protocol/` (34) | `protocol.dict` |
| `volume_surfaces` | `crates/volume-format` | kernel-faithful chain `Record::decode` -> `HostVolumeHeader::decode(payload)`, direct header probe, `encode` producer side | `corpus-seed-volume/` (26) | `volume.dict` |

Anything that panics (indexing, arithmetic, `unwrap` on attacker-controlled
data) or trips ASan is a finding; returning `None`/`Err` is fine.

## Requirements

- **Toolchain**: pinned nightly `nightly-2026-11-02` (see
  `rust-toolchain.toml`; rustup auto-installs it with `rust-src` and
  `llvm-tools-preview` on first use inside `fuzz/`).
- **cargo-fuzz**: `cargo install cargo-fuzz --locked --version 0.13.2`
- **clang + g++** (the libFuzzer runtime is C++).
- The target triple `x86_64-unknown-linux-gnu` is passed explicitly: recent
  cargo-fuzz defaults to musl, which is not what these commands expect.

## Smoke run (bounded, about 60 s per target)

Expected result: libFuzzer runs for 30 s, prints `Done ...` / exec stats and
exits 0; no crash files appear under `fuzz/artifacts/`.

```sh
mkdir -p fuzz/corpus fuzz/artifacts

cargo fuzz run --target x86_64-unknown-linux-gnu --sanitizer address \
  oci_surfaces -- -max_total_time=30 -rss_limit_mb=4096 \
  -artifact_prefix=fuzz/artifacts/ -dict=fuzz/oci.dict \
  fuzz/corpus fuzz/corpus-seed

cargo fuzz run --target x86_64-unknown-linux-gnu --sanitizer address \
  protocol_surfaces -- -max_total_time=30 -rss_limit_mb=4096 \
  -artifact_prefix=fuzz/artifacts/ -dict=fuzz/protocol.dict \
  fuzz/corpus fuzz/corpus-seed-protocol

cargo fuzz run --target x86_64-unknown-linux-gnu --sanitizer address \
  volume_surfaces -- -max_total_time=30 -rss_limit_mb=4096 \
  -artifact_prefix=fuzz/artifacts/ -dict=fuzz/volume.dict \
  fuzz/corpus fuzz/corpus-seed-volume
```

## Longer rounds and coverage

```sh
# 10-minute round per target (same flags, -max_total_time=600)
# Coverage (requires llvm-tools-preview, included in the pinned toolchain):
cargo fuzz coverage --target x86_64-unknown-linux-gnu oci_surfaces -- \
  fuzz/corpus fuzz/corpus-seed
# -> fuzz/coverage/<target>/lcov.info (genhtml for HTML report)
```

## Notes

- Evolved corpora (`fuzz/corpus/`), crash artifacts (`fuzz/artifacts/`) and
  coverage output are git-ignored; only the committed seed corpora and
  dictionaries above are tracked (113 files, < 500 KB total).
- The differential PAX/tar-crate harness explored during preparation is
  deliberately **not** part of this PR: it would add the `tar` crate and its
  transitive dependencies as an external dependency for review-side
  reproduction only.
