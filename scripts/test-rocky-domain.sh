#!/bin/sh
set -eu

. ./scripts/rocky-domain-common.sh

./scripts/prepare-rocky-domain.sh

for artifact in \
    "$ROCKY_DOMAIN_ROOTFS_PATH" \
    "$ROCKY_DOMAIN_MANIFEST_PATH" \
    "$ROCKY_DOMAIN_METADATA_PATH"; do
    test -s "$artifact"
done

archive_list=$ROCKY_DOMAIN_WORK_DIR/archive.list
mkdir -p "$ROCKY_DOMAIN_WORK_DIR"
tar -tf "$ROCKY_DOMAIN_ROOTFS_PATH" > "$archive_list"
for required in \
    ./etc/os-release \
    ./etc/machine-id \
    ./usr/lib/systemd/systemd \
    ./usr/lib64/ld-linux-x86-64.so.2; do
    grep -Fqx "$required" "$archive_list"
done

tar -xOf "$ROCKY_DOMAIN_ROOTFS_PATH" ./usr/lib/os-release |
    grep -Eq '^NAME="?Rocky Linux"?$'
tar -xOf "$ROCKY_DOMAIN_ROOTFS_PATH" ./usr/lib/systemd/systemd |
    file - |
    grep -Fq 'ELF 64-bit LSB pie executable, x86-64'

source_sha=$(shasum -a 256 "$ROCKY_SOURCE_IMAGE" | awk '{print $1}')
manifest_sha=$(shasum -a 256 "$ROCKY_DOMAIN_MANIFEST_PATH" | awk '{print $1}')
grep -Fqx "SOURCE_SHA256=$source_sha" "$ROCKY_DOMAIN_METADATA_PATH"
grep -Fqx "MANIFEST_SHA256=$manifest_sha" "$ROCKY_DOMAIN_METADATA_PATH"

first_manifest=$ROCKY_DOMAIN_WORK_DIR/first.manifest
cp "$ROCKY_DOMAIN_MANIFEST_PATH" "$first_manifest"
GINUS_IMAGE_FORCE=1 ./scripts/prepare-rocky-domain.sh
cmp "$first_manifest" "$ROCKY_DOMAIN_MANIFEST_PATH"

corrupt_source=$ROCKY_DOMAIN_WORK_DIR/corrupt.qcow2
dd if="$ROCKY_SOURCE_IMAGE" of="$corrupt_source" bs=4096 count=1 2>/dev/null
if ROCKY_IMAGE_PATH=$corrupt_source ./scripts/verify-rocky-host.sh >/dev/null 2>&1; then
    printf '%s\n' 'corrupt Rocky source was incorrectly accepted' >&2
    exit 1
fi
if ROCKY_IMAGE_PATH=$ROCKY_DOMAIN_WORK_DIR/missing.qcow2 \
    ./scripts/verify-rocky-host.sh >/dev/null 2>&1; then
    printf '%s\n' 'missing Rocky source was incorrectly accepted' >&2
    exit 1
fi

rm -f "$archive_list" "$first_manifest" "$corrupt_source"
printf '%s\n' 'test-image: PASS'
