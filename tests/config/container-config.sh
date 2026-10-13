#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"

test_dir=$(mktemp -d "${TMPDIR:-/tmp}/ginux-config-test.XXXXXX")
trap 'rm -rf "$test_dir"' EXIT

bad_profile="$test_dir/profile.tsv"
cp profiles/minimal-system-v0/profile.tsv "$bad_profile"
sed -i.bak 's/memory_mib=128/memory_mib=129/' "$bad_profile"

if cargo run --quiet --package ginux-manifest-tool -- \
    boot/manifest-v0.conf \
    control-plane/bootstrap.hex \
    "$test_dir/initramfs.cpio" \
    build/linux-dynamic \
    target/x86_64-unknown-none/release/ginux-controller-runtime \
    target/x86_64-unknown-none/release/ginux-network-service-runtime \
    target/x86_64-unknown-none/release/ginux-protocol-service-runtime \
    target/x86_64-unknown-none/release/ginux-volume-service-runtime \
    "$bad_profile" \
    "$test_dir/generated.rs" >"$test_dir/stdout" 2>"$test_dir/stderr"; then
    echo "Malformed container configuration was accepted" >&2
    exit 1
fi

grep -q "system profile contains a malformed or duplicate row" "$test_dir/stderr"
test ! -e "$test_dir/initramfs.cpio"
test ! -e "$test_dir/generated.rs"
echo "Authenticated container configuration rejection test passed"
