#!/bin/sh
set -eu

# Acquire and materialize one public, digest-pinned Linux OCI image. The cache
# key includes the complete reference and target platform, so unrelated images
# can never share a prepared root.

image_ref=${1-}
cache_root=${2-}
work_root=${3-}
platform_os=${GINUX_OCI_PLATFORM_OS:-linux}
platform_arch=${GINUX_OCI_PLATFORM_ARCH:-amd64}
output=${GINUX_OCI_OUTPUT:-diagnostic}
result_file=${GINUX_OCI_RESULT_FILE:-}

usage() {
    printf '%s\n' 'usage: ensure-oci-image.sh REGISTRY/REPOSITORY@sha256:DIGEST CACHE_ROOT WORK_ROOT' >&2
    exit 64
}

test -n "$image_ref" && test -n "$cache_root" && test -n "$work_root" || usage
case $image_ref in
    *@sha256:*) ;;
    *) printf 'OCI image must use an exact sha256 digest: %s\n' "$image_ref" >&2; exit 65 ;;
esac

repository_ref=${image_ref%@sha256:*}
root_digest=sha256:${image_ref##*@sha256:}
root_hex=${root_digest#sha256:}
case $root_hex in
    ''|*[!0-9a-f]*) printf 'invalid OCI image digest: %s\n' "$root_digest" >&2; exit 65 ;;
esac
test "${#root_hex}" -eq 64 || {
    printf 'invalid OCI image digest: %s\n' "$root_digest" >&2
    exit 65
}

registry=${repository_ref%%/*}
repository=${repository_ref#*/}
test "$registry" != "$repository_ref" && test -n "$repository" || {
    printf 'OCI image reference must include a registry: %s\n' "$image_ref" >&2
    exit 65
}
case $registry in
    docker.io)
        api_registry=registry-1.docker.io
        case $repository in */*) ;; *) repository=library/$repository ;; esac
        ;;
    *) api_registry=$registry ;;
esac
case $registry$repository in
    *[!A-Za-z0-9._:/-]*) printf 'invalid OCI repository: %s\n' "$repository_ref" >&2; exit 65 ;;
esac

for command in curl jq tar gzip awk sed tr wc find; do
    command -v "$command" >/dev/null 2>&1 || {
        printf 'missing required command: %s\n' "$command" >&2
        exit 69
    }
done

sha256_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

sha256_text() {
    if command -v sha256sum >/dev/null 2>&1; then
        printf '%s' "$1" | sha256sum | awk '{print $1}'
    else
        printf '%s' "$1" | shasum -a 256 | awk '{print $1}'
    fi
}

human_bytes() {
    awk -v bytes="$1" 'BEGIN {
        split("B KiB MiB GiB", units, " "); unit=1
        while (bytes >= 1024 && unit < 4) { bytes /= 1024; unit++ }
        if (unit == 1) printf "%.0f %s", bytes, units[unit]
        else printf "%.1f %s", bytes, units[unit]
    }'
}

emit_result() {
    if test -n "$result_file"; then
        printf '%s\n' "$rootfs" > "$result_file"
    else
        printf '%s\n' "$rootfs"
    fi
}

cache_key=$(sha256_text "$image_ref|$platform_os/$platform_arch")
cache_dir=$cache_root/$cache_key
blobs_dir=$cache_dir/blobs/sha256
rootfs=$cache_dir/rootfs.tar
identity=$cache_dir/identity
work_dir=$work_root/$cache_key.$$
headers=$work_dir/headers
body=$work_dir/body
token=

cleanup() {
    rm -rf "$work_dir"
}
trap cleanup 0 HUP INT TERM
mkdir -p "$blobs_dir" "$work_dir"

digest_hex() {
    digest=${1-}
    case $digest in sha256:*) ;; *) return 1 ;; esac
    hex=${digest#sha256:}
    test "${#hex}" -eq 64 || return 1
    case $hex in *[!0-9a-f]*) return 1 ;; esac
    printf '%s\n' "$hex"
}

blob_path() {
    hex=$(digest_hex "$1") || return 1
    printf '%s/%s\n' "$blobs_dir" "$hex"
}

verify_blob() {
    verify_digest=$1
    verify_path=$(blob_path "$verify_digest") || return 1
    test -f "$verify_path" || return 1
    test "sha256:$(sha256_file "$verify_path")" = "$verify_digest"
}

prepared_cache_valid() {
    test -r "$rootfs" && test -r "$identity" || return 1
    grep -Fqx "IMAGE_REF=$image_ref" "$identity" || return 1
    grep -Fqx "PLATFORM=$platform_os/$platform_arch" "$identity" || return 1
    grep -Fqx "ROOT_DIGEST=$root_digest" "$identity" || return 1
    expected_rootfs=$(sed -n 's/^ROOTFS_SHA256=//p' "$identity")
    test -n "$expected_rootfs" || return 1
    test "$(sha256_file "$rootfs")" = "$expected_rootfs"
}

if prepared_cache_valid; then
    test "$output" != human || printf '  Image: %s (%s/%s)\n  Cache: hit\n' \
        "$image_ref" "$platform_os" "$platform_arch"
    emit_result
    exit 0
fi

test "$output" != human || printf '  Image: %s (%s/%s)\n' \
    "$image_ref" "$platform_os" "$platform_arch"

request() {
    request_url=$1
    request_accept=$2
    request_output=$3
    set -- -fsSL --retry 3 -H "Accept: $request_accept"
    test -z "$token" || set -- "$@" -H "Authorization: Bearer $token"
    curl "$@" "$request_url" -o "$request_output"
}

# Discover the public-registry bearer challenge without assuming Docker Hub.
manifest_url=https://$api_registry/v2/$repository/manifests/$root_digest
status=$(curl -sSL --retry 3 -D "$headers" -o "$body" -w '%{http_code}' \
    -H 'Accept: application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json' \
    "$manifest_url")
case $status in
    200) ;;
    401)
        challenge=$(sed -n 's/^[Ww][Ww][Ww]-[Aa]uthenticate:[[:space:]]*[Bb]earer[[:space:]]*//p' "$headers" | tr -d '\r' | head -1)
        realm=$(printf '%s\n' "$challenge" | sed -n 's/.*realm="\([^"]*\)".*/\1/p')
        service=$(printf '%s\n' "$challenge" | sed -n 's/.*service="\([^"]*\)".*/\1/p')
        scope=$(printf '%s\n' "$challenge" | sed -n 's/.*scope="\([^"]*\)".*/\1/p')
        test -n "$realm" || { printf 'unsupported OCI registry authentication challenge\n' >&2; exit 69; }
        set -- -fsSL --retry 3 -G
        test -z "$service" || set -- "$@" --data-urlencode "service=$service"
        if test -n "$scope"; then
            set -- "$@" --data-urlencode "scope=$scope"
        else
            set -- "$@" --data-urlencode "scope=repository:$repository:pull"
        fi
        curl "$@" "$realm" -o "$body"
        token=$(jq -er '.token // .access_token' "$body")
        request "$manifest_url" 'application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json' "$body"
        ;;
    *) printf 'OCI registry returned HTTP %s for %s\n' "$status" "$image_ref" >&2; exit 69 ;;
esac
test "sha256:$(sha256_file "$body")" = "$root_digest" || {
    printf 'OCI root descriptor digest mismatch: %s\n' "$image_ref" >&2
    exit 65
}
root_blob=$(blob_path "$root_digest")
mv "$body" "$root_blob"

root_media=$(jq -er '.mediaType // ""' "$root_blob")
case $root_media in
    application/vnd.oci.image.index.v1+json|application/vnd.docker.distribution.manifest.list.v2+json)
        manifest_digest=$(jq -er --arg os "$platform_os" --arg arch "$platform_arch" \
            '.manifests[] | select(.platform.os == $os and .platform.architecture == $arch and ((.platform.variant // "") == "")) | .digest' \
            "$root_blob" | head -1)
        test -n "$manifest_digest" || {
            printf 'OCI image has no %s/%s manifest: %s\n' "$platform_os" "$platform_arch" "$image_ref" >&2
            exit 65
        }
        manifest_blob=$(blob_path "$manifest_digest")
        if ! verify_blob "$manifest_digest"; then
            request "https://$api_registry/v2/$repository/manifests/$manifest_digest" \
                'application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json' "$body"
            test "sha256:$(sha256_file "$body")" = "$manifest_digest" || {
                printf 'OCI platform manifest digest mismatch: %s\n' "$manifest_digest" >&2
                exit 65
            }
            mv "$body" "$manifest_blob"
        fi
        ;;
    application/vnd.oci.image.manifest.v1+json|application/vnd.docker.distribution.manifest.v2+json|'')
        manifest_digest=$root_digest
        manifest_blob=$root_blob
        ;;
    *) printf 'unsupported OCI root media type: %s\n' "$root_media" >&2; exit 65 ;;
esac

jq -e '.schemaVersion == 2 and (.config.digest | startswith("sha256:")) and all(.layers[]; .digest | startswith("sha256:"))' "$manifest_blob" >/dev/null
config_digest=$(jq -er '.config.digest' "$manifest_blob")
config_blob=$(blob_path "$config_digest")

fetch_blob() {
    fetch_digest=$1
    fetch_size=$2
    fetch_label=$3
    fetch_destination=$(blob_path "$fetch_digest")
    if verify_blob "$fetch_digest"; then return 0; fi
    partial=$work_dir/$(digest_hex "$fetch_digest").partial
    if test "$output" = human; then
        printf '    Downloading %s (%s)...\n' "$fetch_label" "$(human_bytes "$fetch_size")"
    fi
    request "https://$api_registry/v2/$repository/blobs/$fetch_digest" application/octet-stream "$partial"
    test "$(wc -c < "$partial" | tr -d ' ')" -eq "$fetch_size" || {
        printf 'OCI blob size mismatch: %s\n' "$fetch_digest" >&2
        exit 65
    }
    test "sha256:$(sha256_file "$partial")" = "$fetch_digest" || {
        printf 'OCI blob digest mismatch: %s\n' "$fetch_digest" >&2
        exit 65
    }
    mv "$partial" "$fetch_destination"
}

fetch_blob "$config_digest" "$(jq -er '.config.size' "$manifest_blob")" config
config_blob=$(blob_path "$config_digest")
test "$(jq -er '.os' "$config_blob")" = "$platform_os" || {
    printf 'OCI image operating system mismatch\n' >&2; exit 65;
}
test "$(jq -er '.architecture' "$config_blob")" = "$platform_arch" || {
    printf 'OCI image architecture mismatch\n' >&2; exit 65;
}

layer_total=$(jq -er '.layers | length' "$manifest_blob")
test "$layer_total" -gt 0 || { printf 'OCI image contains no layers\n' >&2; exit 65; }
layer_number=0
tab=$(printf '\t')
jq -r '.layers[] | [.digest, (.size | tostring), .mediaType] | @tsv' "$manifest_blob" |
while IFS="$tab" read -r layer_digest layer_size layer_media; do
    layer_number=$((layer_number + 1))
    case $layer_media in
        application/vnd.oci.image.layer.v1.tar+gzip|application/vnd.docker.image.rootfs.diff.tar.gzip|application/vnd.oci.image.layer.v1.tar) ;;
        *) printf 'unsupported OCI layer media type: %s\n' "$layer_media" >&2; exit 65 ;;
    esac
    fetch_blob "$layer_digest" "$layer_size" "layer $layer_number/$layer_total"
done

cache_valid=false
if test -r "$rootfs" && test -r "$identity" && \
   grep -Fqx "IMAGE_REF=$image_ref" "$identity" && \
   grep -Fqx "PLATFORM=$platform_os/$platform_arch" "$identity" && \
   grep -Fqx "MANIFEST_DIGEST=$manifest_digest" "$identity"; then
    expected_rootfs=$(sed -n 's/^ROOTFS_SHA256=//p' "$identity")
    if test -n "$expected_rootfs" && test "$(sha256_file "$rootfs")" = "$expected_rootfs"; then
        cache_valid=true
    fi
fi
if test "$cache_valid" = true; then
    test "$output" != human || printf '  Cache: hit\n'
    emit_result
    exit 0
fi

test "$output" != human || printf '  Cache: miss\n'
merged=$work_dir/merged
mkdir -p "$merged"
layer_list=$work_dir/layers.tsv
jq -r '.layers[] | [.mediaType, .digest] | @tsv' "$manifest_blob" > "$layer_list"
layer_number=0
while IFS="$tab" read -r layer_media layer_digest; do
    layer_blob=$(blob_path "$layer_digest")
    layer_tar=$work_dir/layer-$layer_number.tar
    case $layer_media in
        application/vnd.oci.image.layer.v1.tar+gzip|application/vnd.docker.image.rootfs.diff.tar.gzip) gzip -dc "$layer_blob" > "$layer_tar" ;;
        application/vnd.oci.image.layer.v1.tar) cp "$layer_blob" "$layer_tar" ;;
    esac
    tar -tf "$layer_tar" | while IFS= read -r entry; do
        normalized=${entry#./}
        case /$normalized/ in *'/../'*|*'/./'*|//* ) printf 'unsafe OCI layer path: %s\n' "$entry" >&2; exit 65 ;; esac
        case $normalized in /*) printf 'unsafe absolute OCI layer path: %s\n' "$entry" >&2; exit 65 ;; esac
        base=${normalized##*/}
        parent=${normalized%/*}
        test "$parent" != "$normalized" || parent=.
        case $base in
            .wh..wh..opq)
                test ! -d "$merged/$parent" || find "$merged/$parent" -mindepth 1 -maxdepth 1 -exec rm -rf -- {} +
                ;;
            .wh.*) rm -rf -- "$merged/$parent/${base#.wh.}" ;;
        esac
    done
    tar --exclude='.wh.*' --exclude='*/.wh.*' -xf "$layer_tar" -C "$merged"
    layer_number=$((layer_number + 1))
done < "$layer_list"

rootfs_tmp=$work_dir/rootfs.tar
tar -cf "$rootfs_tmp" -C "$merged" .
rootfs_sha=$(sha256_file "$rootfs_tmp")
identity_tmp=$work_dir/identity
cat > "$identity_tmp" <<EOF
FORMAT_VERSION=1
IMAGE_REF=$image_ref
PLATFORM=$platform_os/$platform_arch
ROOT_DIGEST=$root_digest
MANIFEST_DIGEST=$manifest_digest
CONFIG_DIGEST=$config_digest
ROOTFS_SHA256=$rootfs_sha
EOF
mv "$rootfs_tmp" "$rootfs"
mv "$identity_tmp" "$identity"
test "$output" != human || printf '  Verification: passed\n  Preparation: complete\n'
emit_result
