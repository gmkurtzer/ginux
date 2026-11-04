#!/bin/sh
set -eu

. ./scripts/rocky-domain-common.sh

./scripts/verify-rocky-host.sh
if ! docker image inspect "$ROCKY_DOMAIN_HELPER_IMAGE" >/dev/null 2>&1; then
    printf '%s\n' 'missing pinned image helper; run make image-helper-fetch' >&2
    exit 2
fi

source_sha=$(shasum -a 256 "$ROCKY_SOURCE_IMAGE" | awk '{print $1}')
if [ "${GINUX_IMAGE_FORCE:-0}" != 1 ] &&
    [ -f "$ROCKY_DOMAIN_ROOTFS_PATH" ] &&
    [ -f "$ROCKY_DOMAIN_MANIFEST_PATH" ] &&
    [ -f "$ROCKY_DOMAIN_METADATA_PATH" ] &&
    grep -Fqx "SOURCE_SHA256=$source_sha" "$ROCKY_DOMAIN_METADATA_PATH" &&
    grep -Fqx "HELPER_IMAGE=$ROCKY_DOMAIN_HELPER_IMAGE" "$ROCKY_DOMAIN_METADATA_PATH"; then
    printf 'Rocky domain userspace already prepared: %s\n' "$ROCKY_DOMAIN_ROOTFS_PATH"
    exit 0
fi

mkdir -p "$ROCKY_DOMAIN_CACHE_DIR" "$ROCKY_DOMAIN_WORK_DIR"
raw_image=$ROCKY_DOMAIN_WORK_DIR/source.raw
rootfs_tmp=$ROCKY_DOMAIN_ROOTFS_PATH.tmp
manifest_tmp=$ROCKY_DOMAIN_MANIFEST_PATH.tmp
metadata_tmp=$ROCKY_DOMAIN_METADATA_PATH.tmp

cleanup() {
    rm -f "$raw_image" "$rootfs_tmp" "$manifest_tmp" "$metadata_tmp"
}
trap cleanup EXIT HUP INT TERM

printf '%s\n' 'Converting verified QCOW2 to a temporary sparse raw disk...'
qemu-img convert -f qcow2 -O raw "$ROCKY_SOURCE_IMAGE" "$raw_image"

repository=$(pwd -P)
docker run --rm --privileged --network none \
    --mount "type=bind,source=$repository,target=/workspace" \
    "$ROCKY_DOMAIN_HELPER_IMAGE" \
    /workspace/scripts/extract-rocky-domain.sh \
    /workspace/$raw_image \
    /workspace/$rootfs_tmp \
    /workspace/$manifest_tmp

rootfs_sha=$(shasum -a 256 "$rootfs_tmp" | awk '{print $1}')
manifest_sha=$(shasum -a 256 "$manifest_tmp" | awk '{print $1}')
cat > "$metadata_tmp" <<EOF
FORMAT_VERSION=1
SOURCE_IMAGE=$ROCKY_IMAGE_NAME
SOURCE_SHA256=$source_sha
HELPER_IMAGE=$ROCKY_DOMAIN_HELPER_IMAGE
ROOTFS_SHA256=$rootfs_sha
MANIFEST_SHA256=$manifest_sha
EOF

mv "$rootfs_tmp" "$ROCKY_DOMAIN_ROOTFS_PATH"
mv "$manifest_tmp" "$ROCKY_DOMAIN_MANIFEST_PATH"
mv "$metadata_tmp" "$ROCKY_DOMAIN_METADATA_PATH"
rm -f "$raw_image"
trap - EXIT HUP INT TERM
printf 'Prepared Rocky domain userspace: %s\n' "$ROCKY_DOMAIN_ROOTFS_PATH"
printf 'Content manifest: %s\n' "$ROCKY_DOMAIN_MANIFEST_PATH"
