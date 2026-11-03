#!/bin/sh
set -eu

. ./scripts/oci-common.sh

./scripts/verify-rocky-oci.sh
for command in cargo gzip jq shasum; do
    command -v "$command" >/dev/null 2>&1 || {
        printf 'missing required command: %s\n' "$command" >&2
        exit 1
    }
done

mkdir -p "$GINUS_OCI_WORK_DIR"
manifest_blob=$(oci_blob_path "$GINUS_OCI_MANIFEST_DIGEST")
layer_count=$(jq '.layers | length' "$manifest_blob")
diff_count=$(config_digest=$(jq -er '.config.digest' "$manifest_blob") &&
    config_blob=$(oci_blob_path "$config_digest") && jq '.rootfs.diff_ids | length' "$config_blob")
test "$layer_count" -eq "$diff_count"
if [ "$layer_count" -ne 1 ]; then
    printf 'initial Gate 5 root assembler requires one OCI layer; image has %s\n' \
        "$layer_count" >&2
    exit 1
fi

layer_media_type=$(jq -er '.layers[0].mediaType' "$manifest_blob")
test "$layer_media_type" = application/vnd.oci.image.layer.v1.tar+gzip
layer_digest=$(jq -er '.layers[0].digest' "$manifest_blob")
layer_blob=$(oci_blob_path "$layer_digest")
config_digest=$(jq -er '.config.digest' "$manifest_blob")
config_blob=$(oci_blob_path "$config_digest")
diff_id=$(jq -er '.rootfs.diff_ids[0]' "$config_blob")

rootfs=$GINUS_OCI_CACHE_DIR/$GINUS_OCI_ROOTFS_LAYER
index=$GINUS_OCI_CACHE_DIR/$GINUS_OCI_ROOTFS_INDEX
identity=$GINUS_OCI_CACHE_DIR/$GINUS_OCI_ROOTFS_IDENTITY
rootfs_tmp=$GINUS_OCI_WORK_DIR/rootfs.tar.partial
index_tmp=$GINUS_OCI_WORK_DIR/rootfs.index.partial
identity_tmp=$GINUS_OCI_WORK_DIR/rootfs.env.partial

cleanup() {
    rm -f "$rootfs_tmp" "$index_tmp" "$identity_tmp"
}
trap cleanup 0 HUP INT TERM

if [ ! -f "$rootfs" ] ||
    [ "sha256:$(shasum -a 256 "$rootfs" | awk '{print $1}')" != "$diff_id" ]; then
    rm -f "$rootfs_tmp"
    gzip -dc "$layer_blob" > "$rootfs_tmp"
    actual_diff_id=sha256:$(shasum -a 256 "$rootfs_tmp" | awk '{print $1}')
    if [ "$actual_diff_id" != "$diff_id" ]; then
        printf 'uncompressed OCI layer mismatch: expected %s, got %s\n' \
            "$diff_id" "$actual_diff_id" >&2
        exit 1
    fi
    mv "$rootfs_tmp" "$rootfs"
fi

cargo run -q -p ginus-vfs-index -- "$rootfs" "$index_tmp"
mv "$index_tmp" "$index"
rootfs_sha=$(shasum -a 256 "$rootfs" | awk '{print $1}')
index_sha=$(shasum -a 256 "$index" | awk '{print $1}')
cat > "$identity_tmp" <<EOF
FORMAT_VERSION=1
OCI_INDEX_DIGEST=$GINUS_OCI_INDEX_DIGEST
OCI_MANIFEST_DIGEST=$GINUS_OCI_MANIFEST_DIGEST
OCI_CONFIG_DIGEST=$config_digest
OCI_LAYER_DIGEST=$layer_digest
OCI_LAYER_DIFF_ID=$diff_id
ROOTFS_SHA256=$rootfs_sha
INDEX_SHA256=$index_sha
EOF
mv "$identity_tmp" "$identity"
trap - 0 HUP INT TERM

printf 'OCI_PREPARE: rootfs=%s index=%s diff_id=%s\n' "$rootfs" "$index" "$diff_id"
