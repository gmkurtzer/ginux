#!/bin/sh
set -eu

. ./scripts/oci-common.sh

printf '%s\n' 'Gate 5 OCI image checks:'
./scripts/verify-rocky-oci.sh
printf '%s\n' '  [1/7] PASS  pinned index, manifest, config, and layer digests'

./scripts/prepare-rocky-oci.sh
rootfs=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_LAYER
index=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_INDEX
identity=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_IDENTITY
test -s "$rootfs"
test -s "$index"
test -s "$identity"
printf '%s\n' '  [2/7] PASS  verified OCI layers assembled into immutable Ginux root and index'

first_index_sha=$(shasum -a 256 "$index" | awk '{print $1}')
./scripts/prepare-rocky-oci.sh >/dev/null
second_index_sha=$(shasum -a 256 "$index" | awk '{print $1}')
test "$first_index_sha" = "$second_index_sha"
grep -Fqx "OCI_MANIFEST_DIGEST=$GINUX_OCI_MANIFEST_DIGEST" "$identity"
grep -Fqx "INDEX_SHA256=$second_index_sha" "$identity"
printf '  [3/7] PASS  repeatable index=%s\n' "$second_index_sha"

tar -xOf "$rootfs" usr/lib/os-release | grep -Fq 'Rocky Linux'
tar -xOf "$rootfs" usr/bin/bash | file - | grep -Fq 'ELF 64-bit LSB pie executable, x86-64'
printf '%s\n' '  [4/7] PASS  Rocky identity and x86-64 dynamic shell present'

bad_cache=$GINUX_OCI_WORK_DIR/corrupt-cache
rm -rf "$bad_cache"
mkdir -p "$bad_cache/blobs/sha256"
cp "$GINUX_OCI_CACHE_DIR/oci-layout" "$bad_cache/oci-layout"
cp "$GINUX_OCI_CACHE_DIR/index.json" "$bad_cache/index.json"
index_hex=$(oci_digest_hex "$GINUX_OCI_INDEX_DIGEST")
manifest_hex=$(oci_digest_hex "$GINUX_OCI_MANIFEST_DIGEST")
cp "$GINUX_OCI_BLOBS_DIR/$index_hex" "$bad_cache/blobs/sha256/$index_hex"
printf '%s\n' 'corrupt' > "$bad_cache/blobs/sha256/$manifest_hex"
if GINUX_OCI_CACHE_DIR_OVERRIDE=$bad_cache ./scripts/verify-rocky-oci.sh >/dev/null 2>&1; then
    printf '%s\n' 'corrupt OCI manifest was incorrectly accepted' >&2
    exit 1
fi
if GINUX_OCI_CACHE_DIR_OVERRIDE=$GINUX_OCI_WORK_DIR/missing-cache \
    ./scripts/verify-rocky-oci.sh >/dev/null 2>&1; then
    printf '%s\n' 'missing OCI cache was incorrectly accepted' >&2
    exit 1
fi
rm -rf "$bad_cache"
printf '%s\n' '  [5/7] PASS  corrupt and missing caches rejected'

https_proxy=http://127.0.0.1:1 HTTPS_PROXY=http://127.0.0.1:1 \
    ./scripts/verify-rocky-oci.sh >/dev/null
https_proxy=http://127.0.0.1:1 HTTPS_PROXY=http://127.0.0.1:1 \
    ./scripts/prepare-rocky-oci.sh >/dev/null
printf '%s\n' '  [6/7] PASS  verify and prepare complete with registry access disabled'
./scripts/test-oci-layers.sh >/dev/null
printf '%s\n' '  [7/7] PASS  replacement, whiteout, opaque-directory, and repeatability semantics'
printf '%s\n' 'test-oci-image: PASS (7/7 checks)'
