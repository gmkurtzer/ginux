#!/bin/sh
set -eu

. ./scripts/oci-common.sh

printf '%s\n' 'Gate 5 OCI image checks:'
./scripts/verify-rocky-oci.sh
printf '%s\n' '  [1/6] PASS  pinned index, manifest, config, and layer digests'

./scripts/prepare-rocky-oci.sh
rootfs=$GINUS_OCI_CACHE_DIR/$GINUS_OCI_ROOTFS_LAYER
index=$GINUS_OCI_CACHE_DIR/$GINUS_OCI_ROOTFS_INDEX
identity=$GINUS_OCI_CACHE_DIR/$GINUS_OCI_ROOTFS_IDENTITY
test -s "$rootfs"
test -s "$index"
test -s "$identity"
printf '%s\n' '  [2/6] PASS  verified OCI layer assembled into immutable Ginus root and index'

first_index_sha=$(shasum -a 256 "$index" | awk '{print $1}')
./scripts/prepare-rocky-oci.sh >/dev/null
second_index_sha=$(shasum -a 256 "$index" | awk '{print $1}')
test "$first_index_sha" = "$second_index_sha"
grep -Fqx "OCI_MANIFEST_DIGEST=$GINUS_OCI_MANIFEST_DIGEST" "$identity"
grep -Fqx "INDEX_SHA256=$second_index_sha" "$identity"
printf '  [3/6] PASS  repeatable index=%s\n' "$second_index_sha"

tar -xOf "$rootfs" usr/lib/os-release | grep -Fq 'Rocky Linux'
tar -xOf "$rootfs" usr/bin/bash | file - | grep -Fq 'ELF 64-bit LSB pie executable, x86-64'
printf '%s\n' '  [4/6] PASS  Rocky identity and x86-64 dynamic shell present'

bad_cache=$GINUS_OCI_WORK_DIR/corrupt-cache
rm -rf "$bad_cache"
mkdir -p "$bad_cache/blobs/sha256"
cp "$GINUS_OCI_CACHE_DIR/oci-layout" "$bad_cache/oci-layout"
cp "$GINUS_OCI_CACHE_DIR/index.json" "$bad_cache/index.json"
index_hex=$(oci_digest_hex "$GINUS_OCI_INDEX_DIGEST")
manifest_hex=$(oci_digest_hex "$GINUS_OCI_MANIFEST_DIGEST")
cp "$GINUS_OCI_BLOBS_DIR/$index_hex" "$bad_cache/blobs/sha256/$index_hex"
printf '%s\n' 'corrupt' > "$bad_cache/blobs/sha256/$manifest_hex"
if GINUS_OCI_CACHE_DIR_OVERRIDE=$bad_cache ./scripts/verify-rocky-oci.sh >/dev/null 2>&1; then
    printf '%s\n' 'corrupt OCI manifest was incorrectly accepted' >&2
    exit 1
fi
if GINUS_OCI_CACHE_DIR_OVERRIDE=$GINUS_OCI_WORK_DIR/missing-cache \
    ./scripts/verify-rocky-oci.sh >/dev/null 2>&1; then
    printf '%s\n' 'missing OCI cache was incorrectly accepted' >&2
    exit 1
fi
rm -rf "$bad_cache"
printf '%s\n' '  [5/6] PASS  corrupt and missing caches rejected'

https_proxy=http://127.0.0.1:1 HTTPS_PROXY=http://127.0.0.1:1 \
    ./scripts/verify-rocky-oci.sh >/dev/null
https_proxy=http://127.0.0.1:1 HTTPS_PROXY=http://127.0.0.1:1 \
    ./scripts/prepare-rocky-oci.sh >/dev/null
printf '%s\n' '  [6/6] PASS  verify and prepare complete with registry access disabled'
printf '%s\n' 'test-oci-image: PASS (6/6 checks)'
