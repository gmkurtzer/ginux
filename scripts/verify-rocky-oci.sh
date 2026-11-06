#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
. "$script_dir/oci-common.sh"

for command in jq; do
    command -v "$command" >/dev/null 2>&1 || {
        printf 'missing required command: %s\n' "$command" >&2
        exit 1
    }
done

test "$(jq -er '.imageLayoutVersion' "$GINUX_OCI_CACHE_DIR/oci-layout")" = 1.0.0
index_blob=$(oci_blob_path "$GINUX_OCI_INDEX_DIGEST")
manifest_blob=$(oci_blob_path "$GINUX_OCI_MANIFEST_DIGEST")
oci_verify_blob "$GINUX_OCI_INDEX_DIGEST"
oci_verify_blob "$GINUX_OCI_MANIFEST_DIGEST"

jq -e \
    --arg digest "$GINUX_OCI_MANIFEST_DIGEST" \
    --arg os "$GINUX_OCI_PLATFORM_OS" \
    --arg arch "$GINUX_OCI_PLATFORM_ARCH" \
    'any(.manifests[]; .digest == $digest and .platform.os == $os and
         .platform.architecture == $arch)' \
    "$index_blob" >/dev/null
jq -e --arg digest "$GINUX_OCI_MANIFEST_DIGEST" \
    '.schemaVersion == 2 and .manifests[0].digest == $digest' \
    "$GINUX_OCI_CACHE_DIR/index.json" >/dev/null

config_digest=$(jq -er '.config.digest' "$manifest_blob")
oci_verify_blob "$config_digest"
config_blob=$(oci_blob_path "$config_digest")
config_media_type=$(jq -er '.config.mediaType' "$manifest_blob")
case "$config_media_type" in
    application/vnd.oci.image.config.v1+json|application/vnd.docker.container.image.v1+json) ;;
    *) printf 'unsupported image config media type: %s\n' "$config_media_type" >&2; exit 1 ;;
esac
test "$(jq -er '.config.size' "$manifest_blob")" -eq "$(wc -c < "$config_blob" | tr -d ' ')"
test "$(jq -er '.os' "$config_blob")" = "$GINUX_OCI_PLATFORM_OS"
test "$(jq -er '.architecture' "$config_blob")" = "$GINUX_OCI_PLATFORM_ARCH"

layer_count=0
while IFS=' ' read -r layer_digest layer_size layer_media_type; do
    case "$layer_media_type" in
        application/vnd.oci.image.layer.v1.tar+gzip|application/vnd.docker.image.rootfs.diff.tar.gzip) ;;
        *) printf 'unsupported image layer media type: %s\n' "$layer_media_type" >&2; exit 1 ;;
    esac
    oci_verify_blob "$layer_digest"
    layer_blob=$(oci_blob_path "$layer_digest")
    test "$layer_size" -eq "$(wc -c < "$layer_blob" | tr -d ' ')"
    layer_count=$((layer_count + 1))
done <<EOF
$(jq -r '.layers[] | [.digest, (.size | tostring), .mediaType] | join(" ")' "$manifest_blob")
EOF
test "$layer_count" -gt 0

printf 'OCI_VERIFY: image=docker.io/%s:%s platform=%s/%s layers=%s manifest=%s\n' \
    "$GINUX_OCI_REPOSITORY" "$GINUX_OCI_TAG" "$GINUX_OCI_PLATFORM_OS" \
    "$GINUX_OCI_PLATFORM_ARCH" "$layer_count" "$GINUX_OCI_MANIFEST_DIGEST"
