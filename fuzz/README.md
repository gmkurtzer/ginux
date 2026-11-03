# fuzz/ — libFuzzer harnesses for the untrusted-input parser surfaces

A standalone cargo-fuzz workspace covering the three crates that parse bytes
derived from untrusted input. This directory is intentionally **not** a member
of the root workspace: upstream crates keep resolving `license.workspace =
true` against the root manifest, and no root file is modified.

All commands below are run from **inside this directory** (`cd fuzz`), so the
pinned toolchain in `rust-toolchain.toml` applies and the root toolchain
(1.99.0, UEFI target) is untouched. The commands use the host's default
target, so they run natively on Linux and macOS.

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
  `llvm-tools-preview` on first use from inside `fuzz/`).
- **cargo-fuzz**: `cargo install cargo-fuzz --locked --version 0.13.2`
- **clang and a C++ compiler** (the libFuzzer runtime is C++): on Linux
  `clang` and `g++`; on macOS the Xcode command line tools.

## Smoke run (bounded, about 60 s per target)

Expected result: libFuzzer runs for 30 s, prints `Done ...` / exec stats and
exits 0; no crash files appear under `fuzz/artifacts/`.

```sh
cd fuzz
mkdir -p corpus artifacts

cargo fuzz run --sanitizer address \
  oci_surfaces -- -max_total_time=30 -rss_limit_mb=4096 \
  -artifact_prefix=artifacts/ -dict=oci.dict \
  corpus corpus-seed

cargo fuzz run --sanitizer address \
  protocol_surfaces -- -max_total_time=30 -rss_limit_mb=4096 \
  -artifact_prefix=artifacts/ -dict=protocol.dict \
  corpus corpus-seed-protocol

cargo fuzz run --sanitizer address \
  volume_surfaces -- -max_total_time=30 -rss_limit_mb=4096 \
  -artifact_prefix=artifacts/ -dict=volume.dict \
  corpus corpus-seed-volume
```

## Longer rounds and coverage

```sh
# 10-minute round per target: same flags, -max_total_time=600

# Coverage (llvm-tools-preview is included in the pinned toolchain):
cargo fuzz coverage oci_surfaces corpus corpus-seed

# cargo-fuzz 0.13.2 stops at the merged profile; summarise with llvm-cov
# from the pinned toolchain. The coverage build lives under a separate
# target dir: target/<host>/coverage/<host>/release/
HOST="$(rustc -vV | sed -n 's/^host: //p')"
"$(rustc --print sysroot)/lib/rustlib/$HOST/bin/llvm-cov" report \
  "target/$HOST/coverage/$HOST/release/oci_surfaces" \
  -instr-profile=coverage/oci_surfaces/coverage.profdata -summary-only
# -> coverage/oci_surfaces/coverage.profdata + the per-file/TOTAL report table
```

## Optional: explicit Linux target

The commands above use the host's default target. To force a Linux target
(for example when cross-building from another host), append
`--target x86_64-unknown-linux-gnu` to the `cargo fuzz` commands. This
additionally requires `x86_64-linux-gnu-g++` as a cross compiler, and the
resulting executable only runs on a Linux x86_64 host. If you use it for
coverage, replace `$HOST` in the paths above with `x86_64-unknown-linux-gnu`.

## Notes

- Evolved corpora (`fuzz/corpus/`), crash artifacts (`fuzz/artifacts/`) and
  coverage output are git-ignored; only the committed seed corpora and
  dictionaries above are tracked (113 files, about 84 KB total).
- The differential PAX/tar-crate harness explored during preparation is
  deliberately **not** part of this PR: it would add the `tar` crate and its
  transitive dependencies as an external dependency for review-side
  reproduction only.
