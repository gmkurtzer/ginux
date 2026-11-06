#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
. "$script_dir/oci-common.sh"
"$script_dir/verify-rocky-oci.sh"

for command in gzip jq tar; do
    command -v "$command" >/dev/null 2>&1 || {
        printf 'missing required command: %s\n' "$command" >&2
        exit 1
    }
done

mkdir -p "$GINUX_OCI_WORK_DIR" "$GINUX_OCI_CACHE_DIR"
manifest_blob=$(oci_blob_path "$GINUX_OCI_MANIFEST_DIGEST")
config_digest=$(jq -er '.config.digest' "$manifest_blob")
config_blob=$(oci_blob_path "$config_digest")
layer_count=$(jq '.layers | length' "$manifest_blob")
diff_count=$(jq '.rootfs.diff_ids | length' "$config_blob")
test "$layer_count" -eq "$diff_count"

rootfs=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_LAYER
identity=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_IDENTITY
merged=$GINUX_OCI_WORK_DIR/merged-root
rootfs_tmp=$GINUX_OCI_WORK_DIR/rootfs.tar.partial
identity_tmp=$GINUX_OCI_WORK_DIR/rootfs.env.partial
layer_list=$GINUX_OCI_WORK_DIR/layers.tsv

cleanup() {
    rm -rf "$merged"
    rm -f "$rootfs_tmp" "$identity_tmp" "$layer_list" \
        "$layer_list.media" "$layer_list.diff" "$GINUX_OCI_WORK_DIR"/layer-*.tar
}
trap cleanup 0 HUP INT TERM
rm -rf "$merged"
mkdir -p "$merged"

jq -r '.layers[] | [.mediaType, .digest] | @tsv' "$manifest_blob" >"$layer_list.media"
jq -r '.rootfs.diff_ids[]' "$config_blob" >"$layer_list.diff"
paste "$layer_list.media" "$layer_list.diff" >"$layer_list"

layer_number=0
tab=$(printf '\t')
while IFS="$tab" read -r layer_media_type layer_digest diff_id; do
    case "$layer_media_type" in
        application/vnd.oci.image.layer.v1.tar+gzip|application/vnd.docker.image.rootfs.diff.tar.gzip) ;;
        *) printf 'unsupported image layer media type: %s\n' "$layer_media_type" >&2; exit 1 ;;
    esac
    layer_blob=$(oci_blob_path "$layer_digest")
    layer_tar=$GINUX_OCI_WORK_DIR/layer-$layer_number.tar
    gzip -dc "$layer_blob" >"$layer_tar"
    actual_diff_id=sha256:$(oci_sha256_file "$layer_tar")
    if test "$actual_diff_id" != "$diff_id"; then
        printf 'uncompressed OCI layer mismatch: expected %s, got %s\n' \
            "$diff_id" "$actual_diff_id" >&2
        exit 1
    fi

    tar -tf "$layer_tar" | while IFS= read -r entry; do
        normalized=${entry#./}
        case "/$normalized/" in
            *'/../'*|*'/./'*|//* )
                printf 'unsafe OCI layer path: %s\n' "$entry" >&2
                exit 1
                ;;
        esac
        base=${normalized##*/}
        parent=${normalized%/*}
        test "$parent" != "$normalized" || parent=.
        case "$base" in
            .wh..wh..opq)
                if test -d "$merged/$parent"; then
                    find "$merged/$parent" -mindepth 1 -maxdepth 1 -exec rm -rf -- {} +
                fi
                ;;
            .wh.*)
                target=${base#.wh.}
                rm -rf -- "$merged/$parent/$target"
                ;;
        esac
    done
    tar --numeric-owner --same-owner --exclude='.wh.*' --exclude='*/.wh.*' \
        -xf "$layer_tar" -C "$merged"
    layer_number=$((layer_number + 1))
done <"$layer_list"
test "$layer_number" -eq "$layer_count"

tar --numeric-owner -cf "$rootfs_tmp" -C "$merged" .
mv "$rootfs_tmp" "$rootfs"
rootfs_sha=$(oci_sha256_file "$rootfs")
cat >"$identity_tmp" <<EOF
FORMAT_VERSION=1
OCI_INDEX_DIGEST=$GINUX_OCI_INDEX_DIGEST
OCI_MANIFEST_DIGEST=$GINUX_OCI_MANIFEST_DIGEST
OCI_CONFIG_DIGEST=$config_digest
OCI_LAYER_COUNT=$layer_count
ROOTFS_SHA256=$rootfs_sha
EOF
mv "$identity_tmp" "$identity"
trap - 0 HUP INT TERM
cleanup
printf 'OCI_PREPARE: rootfs=%s layers=%s whiteouts=applied\n' "$rootfs" "$layer_count"
