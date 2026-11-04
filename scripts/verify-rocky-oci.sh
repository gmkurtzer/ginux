#!/bin/sh
set -eu

. ./scripts/oci-common.sh

for command in jq shasum; do
    command -v "$command" >/dev/null 2>&1 || {
        printf 'missing required command: %s\n' "$command" >&2
        exit 1
    }
done

test "$(jq -er '.imageLayoutVersion' "$GINUS_OCI_CACHE_DIR/oci-layout")" = 1.0.0
index_blob=$(oci_blob_path "$GINUS_OCI_INDEX_DIGEST")
manifest_blob=$(oci_blob_path "$GINUS_OCI_MANIFEST_DIGEST")
oci_verify_blob "$GINUS_OCI_INDEX_DIGEST"
oci_verify_blob "$GINUS_OCI_MANIFEST_DIGEST"

jq -e \
    --arg digest "$GINUS_OCI_MANIFEST_DIGEST" \
    --arg os "$GINUS_OCI_PLATFORM_OS" \
    --arg arch "$GINUS_OCI_PLATFORM_ARCH" \
    'any(.manifests[]; .digest == $digest and .platform.os == $os and
         .platform.architecture == $arch)' \
    "$index_blob" >/dev/null
jq -e --arg digest "$GINUS_OCI_MANIFEST_DIGEST" \
    '.schemaVersion == 2 and .manifests[0].digest == $digest' \
    "$GINUS_OCI_CACHE_DIR/index.json" >/dev/null

config_digest=$(jq -er '.config.digest' "$manifest_blob")
oci_verify_blob "$config_digest"
config_blob=$(oci_blob_path "$config_digest")
test "$(jq -er '.config.mediaType' "$manifest_blob")" = application/vnd.oci.image.config.v1+json
test "$(jq -er '.config.size' "$manifest_blob")" -eq "$(wc -c < "$config_blob" | tr -d ' ')"
test "$(jq -er '.os' "$config_blob")" = "$GINUS_OCI_PLATFORM_OS"
test "$(jq -er '.architecture' "$config_blob")" = "$GINUS_OCI_PLATFORM_ARCH"

layer_count=0
while IFS=' ' read -r layer_digest layer_size layer_media_type; do
    test "$layer_media_type" = application/vnd.oci.image.layer.v1.tar+gzip
    oci_verify_blob "$layer_digest"
    layer_blob=$(oci_blob_path "$layer_digest")
    test "$layer_size" -eq "$(wc -c < "$layer_blob" | tr -d ' ')"
    layer_count=$((layer_count + 1))
done <<EOF
$(jq -r '.layers[] | [.digest, (.size | tostring), .mediaType] | join(" ")' "$manifest_blob")
EOF
test "$layer_count" -gt 0

printf 'OCI_VERIFY: image=docker.io/%s:%s platform=%s/%s layers=%s manifest=%s\n' \
    "$GINUS_OCI_REPOSITORY" "$GINUS_OCI_TAG" "$GINUS_OCI_PLATFORM_OS" \
    "$GINUS_OCI_PLATFORM_ARCH" "$layer_count" "$GINUS_OCI_MANIFEST_DIGEST"
