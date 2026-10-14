#!/usr/bin/env bash
set -euo pipefail

repository_root=$(cd "$(dirname "$0")/../.." && pwd)
profile="$repository_root/profiles/alpine-3.24-shell-v0/image.env"
output_directory="$repository_root/build/alpine-3.24-shell-v0"
oci_archive="$output_directory/oci-layout.tar"
rootfs="$output_directory/rootfs.tar"
metadata="$output_directory/image.tsv"

# shellcheck source=/dev/null
source "$profile"

reference="$ALPINE_IMAGE_REPOSITORY@$ALPINE_IMAGE_DIGEST"
manifest_blob="blobs/sha256/${ALPINE_IMAGE_DIGEST#sha256:}"
config_blob="blobs/sha256/${ALPINE_IMAGE_CONFIG_DIGEST#sha256:}"
layer_blob="blobs/sha256/${ALPINE_IMAGE_LAYER_DIGEST#sha256:}"

fail() {
    echo "alpine-image: $*" >&2
    exit 1
}

require_tool() {
    command -v "$1" >/dev/null 2>&1 || fail "missing required tool: $1"
}

hash_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

verify_blob() {
    local path=$1 expected=$2 actual
    [[ -s "$path" ]] || fail "missing OCI object $path"
    actual=$(hash_file "$path")
    [[ "sha256:$actual" == "$expected" ]] || fail "digest mismatch for $path"
}

verify_rootfs() {
    [[ -s "$rootfs" ]] || fail "missing $rootfs; run 'make alpine-image' first"
    [[ "sha256:$(hash_file "$rootfs")" == "$ALPINE_IMAGE_ROOTFS_DIGEST" ]] ||
        fail "root filesystem digest mismatch"
    for path in bin/sh bin/busybox lib/ld-musl-x86_64.so.1 etc/alpine-release; do
        tar -tf "$rootfs" "$path" >/dev/null 2>&1 || fail "root filesystem is missing $path"
    done
}

verify_archive() {
    local temporary manifest config layer
    [[ -s "$oci_archive" ]] || fail "missing $oci_archive; run 'make alpine-image' first"
    temporary=$(mktemp -d "${TMPDIR:-/tmp}/ginux-alpine-check.XXXXXX")
    trap 'rm -rf "$temporary"' RETURN
    manifest="$temporary/manifest"
    config="$temporary/config"
    layer="$temporary/layer"
    tar -xOf "$oci_archive" "$manifest_blob" >"$manifest"
    tar -xOf "$oci_archive" "$config_blob" >"$config"
    tar -xOf "$oci_archive" "$layer_blob" >"$layer"
    verify_blob "$manifest" "$ALPINE_IMAGE_DIGEST"
    verify_blob "$config" "$ALPINE_IMAGE_CONFIG_DIGEST"
    verify_blob "$layer" "$ALPINE_IMAGE_LAYER_DIGEST"
    grep -Fq '"architecture":"amd64"' "$config" || fail "OCI config is not amd64"
    grep -Fq '"os":"linux"' "$config" || fail "OCI config is not Linux"
    grep -Fq '"Cmd":["/bin/sh"]' "$config" || fail "OCI config does not select /bin/sh"
    grep -Fq "$ALPINE_IMAGE_CONFIG_DIGEST" "$manifest" || fail "manifest config mismatch"
    grep -Fq "$ALPINE_IMAGE_LAYER_DIGEST" "$manifest" || fail "manifest layer mismatch"
    trap - RETURN
    rm -rf "$temporary"
}

write_metadata() {
    {
        printf 'GINUX_ALPINE_IMAGE_V0\n'
        printf 'source\t%s:%s\n' "$ALPINE_IMAGE_REPOSITORY" "$ALPINE_IMAGE_TAG"
        printf 'manifest\t%s\n' "$ALPINE_IMAGE_DIGEST"
        printf 'config\t%s\n' "$ALPINE_IMAGE_CONFIG_DIGEST"
        printf 'layer\t%s\n' "$ALPINE_IMAGE_LAYER_DIGEST"
        printf 'rootfs\t%s\n' "$ALPINE_IMAGE_ROOTFS_DIGEST"
        printf 'platform\t%s\n' "$ALPINE_IMAGE_PLATFORM"
        printf 'init\t%s\n' "$ALPINE_IMAGE_INIT"
    } >"$metadata"
}

case "${1:-}" in
    fetch)
        require_tool docker
        require_tool gzip
        require_tool tar
        mkdir -p "$output_directory"
        docker pull --platform "$ALPINE_IMAGE_PLATFORM" "$reference"
        identity=$(docker image inspect "$reference" --format '{{index .RepoDigests 0}}')
        platform=$(docker image inspect "$reference" --format '{{.Os}}/{{.Architecture}}')
        command=$(docker image inspect "$reference" --format '{{join .Config.Cmd " "}}')
        [[ "$identity" == "$reference" ]] || fail "local image identity does not match $reference"
        [[ "$platform" == "$ALPINE_IMAGE_PLATFORM" ]] || fail "expected $ALPINE_IMAGE_PLATFORM, found $platform"
        [[ "$command" == "$ALPINE_IMAGE_INIT" ]] || fail "expected image command $ALPINE_IMAGE_INIT, found $command"
        docker save "$reference" --output "$oci_archive"
        verify_archive
        tar -xOf "$oci_archive" "$layer_blob" | gzip -dc >"$rootfs"
        verify_rootfs
        write_metadata
        echo "Prepared verified Alpine OCI image at $oci_archive"
        echo "Prepared verified Alpine root filesystem at $rootfs"
        ;;
    check)
        require_tool tar
        verify_archive
        verify_rootfs
        [[ -s "$metadata" ]] || fail "missing $metadata; run 'make alpine-image' first"
        grep -Fqx $'manifest\t'"$ALPINE_IMAGE_DIGEST" "$metadata" || fail "metadata manifest mismatch"
        grep -Fqx $'platform\t'"$ALPINE_IMAGE_PLATFORM" "$metadata" || fail "metadata platform mismatch"
        grep -Fqx $'init\t'"$ALPINE_IMAGE_INIT" "$metadata" || fail "metadata init mismatch"
        echo "Verified pinned Alpine OCI image and root filesystem"
        ;;
    *)
        fail "usage: $0 fetch|check"
        ;;
esac
