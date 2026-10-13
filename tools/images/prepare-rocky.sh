#!/usr/bin/env bash
set -euo pipefail

repository_root=$(cd "$(dirname "$0")/../.." && pwd)
profile="$repository_root/profiles/rocky-10-system-v0/image.env"
output_directory="$repository_root/build/rocky-10-system-v0"
rootfs="$output_directory/rootfs.tar"
metadata="$output_directory/image.tsv"

# shellcheck source=/dev/null
source "$profile"

reference="$ROCKY_IMAGE_REPOSITORY@$ROCKY_IMAGE_DIGEST"
container="ginux-rocky-image-$$"

fail() {
    echo "rocky-image: $*" >&2
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

verify_local_image() {
    local identity architecture os command
    identity=$(docker image inspect "$reference" --format '{{index .RepoDigests 0}}')
    architecture=$(docker image inspect "$reference" --format '{{.Architecture}}')
    os=$(docker image inspect "$reference" --format '{{.Os}}')
    command=$(docker image inspect "$reference" --format '{{join .Config.Cmd " "}}')
    [[ "$identity" == "$reference" ]] || fail "local image digest does not match $reference"
    [[ "$os/$architecture" == "$ROCKY_IMAGE_PLATFORM" ]] || fail "expected $ROCKY_IMAGE_PLATFORM, found $os/$architecture"
    [[ "$command" == "$ROCKY_IMAGE_INIT" ]] || fail "expected image command $ROCKY_IMAGE_INIT, found $command"
}

verify_rootfs() {
    [[ -s "$rootfs" ]] || fail "missing $rootfs; run 'make rocky-image' first"
    for path in sbin usr/sbin/init usr/lib/systemd/systemd usr/lib64/ld-linux-x86-64.so.2 usr/lib/os-release; do
        tar -tf "$rootfs" "$path" >/dev/null 2>&1 || fail "root filesystem is missing $path"
    done
}

write_metadata() {
    local rootfs_digest
    rootfs_digest=$(hash_file "$rootfs")
    {
        printf 'GINUX_ROCKY_IMAGE_V0\n'
        printf 'source\t%s:%s\n' "$ROCKY_IMAGE_REPOSITORY" "$ROCKY_IMAGE_TAG"
        printf 'manifest\t%s\n' "$ROCKY_IMAGE_DIGEST"
        printf 'platform\t%s\n' "$ROCKY_IMAGE_PLATFORM"
        printf 'init\t%s\n' "$ROCKY_IMAGE_INIT"
        printf 'rootfs-sha256\t%s\n' "$rootfs_digest"
        printf 'rootfs-bytes\t%s\n' "$(wc -c < "$rootfs" | tr -d ' ')"
        printf 'systemd\tpresent\n'
    } > "$metadata"
}

case "${1:-}" in
    fetch)
        require_tool docker
        require_tool tar
        mkdir -p "$output_directory"
        docker pull --platform "$ROCKY_IMAGE_PLATFORM" "$reference"
        verify_local_image
        trap 'docker rm -f "$container" >/dev/null 2>&1 || true' EXIT
        docker create --platform "$ROCKY_IMAGE_PLATFORM" --name "$container" "$reference" >/dev/null
        docker export --output "$rootfs" "$container"
        verify_rootfs
        write_metadata
        echo "Prepared $rootfs"
        echo "Verified $reference ($ROCKY_IMAGE_PLATFORM, init $ROCKY_IMAGE_INIT)"
        ;;
    check)
        require_tool tar
        verify_rootfs
        [[ -s "$metadata" ]] || fail "missing $metadata; run 'make rocky-image' first"
        grep -Fqx $'manifest\t'"$ROCKY_IMAGE_DIGEST" "$metadata" || fail "metadata manifest mismatch"
        grep -Fqx $'platform\t'"$ROCKY_IMAGE_PLATFORM" "$metadata" || fail "metadata platform mismatch"
        grep -Fqx $'init\t'"$ROCKY_IMAGE_INIT" "$metadata" || fail "metadata init mismatch"
        grep -Fqx $'rootfs-sha256\t'"$(hash_file "$rootfs")" "$metadata" || fail "root filesystem digest mismatch"
        echo "Verified pinned Rocky root filesystem at $rootfs"
        ;;
    *)
        fail "usage: $0 fetch|check"
        ;;
esac
