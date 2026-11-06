#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
. "$script_dir/oci-common.sh"
umask 077

for command in curl jq; do
    command -v "$command" >/dev/null 2>&1 || {
        printf 'missing required command: %s\n' "$command" >&2
        exit 1
    }
done

mkdir -p "$GINUX_OCI_BLOBS_DIR" "$GINUX_OCI_WORK_DIR"
token_file=$GINUX_OCI_WORK_DIR/token.json
layout_tmp=$GINUX_OCI_WORK_DIR/oci-layout.tmp
index_tmp=$GINUX_OCI_WORK_DIR/index.json.tmp

cleanup() {
    rm -f "$token_file" "$layout_tmp" "$index_tmp" "$GINUX_OCI_WORK_DIR"/*.partial
}
trap cleanup 0 HUP INT TERM

curl -fsSL --retry 3 \
    "https://auth.docker.io/token?service=$GINUX_OCI_AUTH_SERVICE&scope=repository:$GINUX_OCI_REPOSITORY:pull" \
    -o "$token_file"
token=$(jq -er '.token' "$token_file")

fetch_object() {
    endpoint=$1
    digest=$2
    accept=$3
    label=${4:-object}
    size=${5:-unknown}
    destination=$(oci_blob_path "$digest")
    if oci_verify_blob "$digest"; then
        if [ "$label" != object ]; then
            printf 'OCI_FETCH: cached %s digest=%s size=%s\n' "$label" "$digest" "$size"
        fi
        return 0
    fi
    partial=$GINUX_OCI_WORK_DIR/$(oci_digest_hex "$digest").partial
    rm -f "$partial"
    if [ "$label" = object ]; then
        curl -fsSL --retry 3 \
            -H "Authorization: Bearer $token" \
            -H "Accept: $accept" \
            "https://$GINUX_OCI_REGISTRY/v2/$GINUX_OCI_REPOSITORY/$endpoint/$digest" \
            -o "$partial"
    else
        printf 'OCI_FETCH: downloading %s digest=%s size=%s bytes\n' "$label" "$digest" "$size"
        curl -fL --retry 3 --progress-bar --show-error \
            -H "Authorization: Bearer $token" \
            -H "Accept: $accept" \
            "https://$GINUX_OCI_REGISTRY/v2/$GINUX_OCI_REPOSITORY/$endpoint/$digest" \
            -o "$partial"
        printf 'OCI_FETCH: downloaded %s digest=%s size=%s bytes\n' "$label" "$digest" "$size"
    fi
    actual=$(oci_sha256_file "$partial")
    if [ "sha256:$actual" != "$digest" ]; then
        printf 'digest mismatch for %s: expected %s, got sha256:%s\n' \
            "$endpoint" "$digest" "$actual" >&2
        exit 1
    fi
    mv "$partial" "$destination"
}

fetch_object manifests "$GINUX_OCI_INDEX_DIGEST" "application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json"
index_blob=$(oci_blob_path "$GINUX_OCI_INDEX_DIGEST")
jq -e \
    --arg digest "$GINUX_OCI_MANIFEST_DIGEST" \
    --arg os "$GINUX_OCI_PLATFORM_OS" \
    --arg arch "$GINUX_OCI_PLATFORM_ARCH" \
    '.schemaVersion == 2 and
     (.mediaType == "application/vnd.oci.image.index.v1+json" or
      .mediaType == "application/vnd.docker.distribution.manifest.list.v2+json") and
     any(.manifests[]; .digest == $digest and .platform.os == $os and
         .platform.architecture == $arch)' \
    "$index_blob" >/dev/null

fetch_object manifests "$GINUX_OCI_MANIFEST_DIGEST" "application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json"
manifest_blob=$(oci_blob_path "$GINUX_OCI_MANIFEST_DIGEST")
jq -e '.schemaVersion == 2 and
       (.mediaType == "application/vnd.oci.image.manifest.v1+json" or
        .mediaType == "application/vnd.docker.distribution.manifest.v2+json") and
       (.config.digest | startswith("sha256:")) and
       all(.layers[]; .digest | startswith("sha256:"))' \
    "$manifest_blob" >/dev/null

config_digest=$(jq -er '.config.digest' "$manifest_blob")
config_size=$(jq -er '.config.size' "$manifest_blob")
fetch_object blobs "$config_digest" application/octet-stream config "$config_size"

layer_total=$(jq -er '.layers | length' "$manifest_blob")
layer_number=0
tab=$(printf '\t')
jq -r '.layers[] | [.digest, .size] | @tsv' "$manifest_blob" |
while IFS="$tab" read -r layer_digest layer_size; do
    layer_number=$((layer_number + 1))
    fetch_object blobs "$layer_digest" application/octet-stream \
        "layer $layer_number/$layer_total" "$layer_size"
done

manifest_size=$(wc -c < "$manifest_blob" | tr -d ' ')
manifest_media_type=$(jq -er '.mediaType' "$manifest_blob")
printf '%s\n' '{"imageLayoutVersion":"1.0.0"}' > "$layout_tmp"
jq -n \
    --arg digest "$GINUX_OCI_MANIFEST_DIGEST" \
    --argjson size "$manifest_size" \
    --arg media_type "$manifest_media_type" \
    --arg ref "docker.io/$GINUX_OCI_REPOSITORY:$GINUX_OCI_TAG" \
    --arg os "$GINUX_OCI_PLATFORM_OS" \
    --arg arch "$GINUX_OCI_PLATFORM_ARCH" \
    '{schemaVersion: 2, manifests: [{
      mediaType: $media_type,
      digest: $digest,
      size: $size,
      annotations: {"org.opencontainers.image.ref.name": $ref},
      platform: {os: $os, architecture: $arch}
    }]}' > "$index_tmp"
mv "$layout_tmp" "$GINUX_OCI_CACHE_DIR/oci-layout"
mv "$index_tmp" "$GINUX_OCI_CACHE_DIR/index.json"

"$script_dir/verify-rocky-oci.sh"
printf 'OCI_FETCH: cached=%s manifest=%s\n' \
    "$GINUX_OCI_CACHE_DIR" "$GINUX_OCI_MANIFEST_DIGEST"
