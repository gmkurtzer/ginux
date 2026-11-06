#!/bin/sh
set -eu

. "$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)/oci-common.sh"

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
"$script_dir/verify-rocky-oci.sh"
for command in gzip jq paste; do
    command -v "$command" >/dev/null 2>&1 || {
        printf 'missing required command: %s\n' "$command" >&2
        exit 1
    }
done

mkdir -p "$GINUX_OCI_WORK_DIR"
manifest_blob=$(oci_blob_path "$GINUX_OCI_MANIFEST_DIGEST")
layer_count=$(jq '.layers | length' "$manifest_blob")
diff_count=$(config_digest=$(jq -er '.config.digest' "$manifest_blob") &&
    config_blob=$(oci_blob_path "$config_digest") && jq '.rootfs.diff_ids | length' "$config_blob")
test "$layer_count" -eq "$diff_count"
config_digest=$(jq -er '.config.digest' "$manifest_blob")
config_blob=$(oci_blob_path "$config_digest")

rootfs=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_LAYER
index=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_INDEX
identity=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_IDENTITY
rootfs_tmp=$GINUX_OCI_WORK_DIR/rootfs.tar.partial
index_tmp=$GINUX_OCI_WORK_DIR/rootfs.index.partial
identity_tmp=$GINUX_OCI_WORK_DIR/rootfs.env.partial
layer_list=$GINUX_OCI_WORK_DIR/layers.tsv

cleanup() {
    rm -f "$rootfs_tmp" "$index_tmp" "$identity_tmp" "$layer_list" \
        "$layer_list.media" "$layer_list.diff" "$GINUX_OCI_WORK_DIR"/*.partial
}
trap cleanup 0 HUP INT TERM

jq -r '.layers[] | [.mediaType, .digest] | @tsv' "$manifest_blob" > "$layer_list.media"
jq -r '.rootfs.diff_ids[]' "$config_blob" > "$layer_list.diff"
paste "$layer_list.media" "$layer_list.diff" > "$layer_list"
rm -f "$layer_list.media" "$layer_list.diff"

set --
layer_number=0
tab=$(printf '\t')
while IFS="$tab" read -r layer_media_type layer_digest diff_id; do
    case "$layer_media_type" in
        application/vnd.oci.image.layer.v1.tar+gzip|application/vnd.docker.image.rootfs.diff.tar.gzip) ;;
        *) printf 'unsupported image layer media type: %s\n' "$layer_media_type" >&2; exit 1 ;;
    esac
    layer_blob=$(oci_blob_path "$layer_digest")
    layer_tar=$GINUX_OCI_WORK_DIR/layer-$layer_number.tar
    layer_tmp=$layer_tar.partial
    gzip -dc "$layer_blob" > "$layer_tmp"
    actual_diff_id=sha256:$(oci_sha256_file "$layer_tmp")
    if [ "$actual_diff_id" != "$diff_id" ]; then
        printf 'uncompressed OCI layer mismatch: expected %s, got %s\n' \
            "$diff_id" "$actual_diff_id" >&2
        exit 1
    fi
    mv "$layer_tmp" "$layer_tar"
    set -- "$@" "$layer_tar"
    layer_number=$((layer_number + 1))
done < "$layer_list"
test "$layer_number" -eq "$layer_count"

if test -n "${GINUX_VFS_INDEX-}"; then
    "$GINUX_VFS_INDEX" assemble "$rootfs_tmp" "$index_tmp" "$@"
else
    command -v cargo >/dev/null 2>&1 || {
        printf '%s\n' 'missing required command: cargo' >&2
        exit 1
    }
    cargo run -q -p ginux-vfs-index -- assemble "$rootfs_tmp" "$index_tmp" "$@"
fi
mv "$rootfs_tmp" "$rootfs"
mv "$index_tmp" "$index"
rootfs_sha=$(oci_sha256_file "$rootfs")
index_sha=$(oci_sha256_file "$index")
cat > "$identity_tmp" <<EOF
FORMAT_VERSION=1
OCI_INDEX_DIGEST=$GINUX_OCI_INDEX_DIGEST
OCI_MANIFEST_DIGEST=$GINUX_OCI_MANIFEST_DIGEST
OCI_CONFIG_DIGEST=$config_digest
OCI_LAYER_COUNT=$layer_count
ROOTFS_SHA256=$rootfs_sha
INDEX_SHA256=$index_sha
EOF
mv "$identity_tmp" "$identity"
trap - 0 HUP INT TERM

printf 'OCI_PREPARE: rootfs=%s index=%s layers=%s\n' "$rootfs" "$index" "$layer_count"
