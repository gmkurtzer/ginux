#!/bin/sh
set -eu

test_root=$(mktemp -d "${TMPDIR:-/tmp}/ginux-generic-oci.XXXXXX")
cleanup() {
    rm -rf "$test_root"
}
trap cleanup 0 HUP INT TERM

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

reference=docker.io/library/alpine@sha256:9db2934886712d877c932e07249c75a59e6ca27adfbccc05261987a17bef2d63
other_reference=docker.io/library/ubuntu@sha256:9db2934886712d877c932e07249c75a59e6ca27adfbccc05261987a17bef2d63
key=$(sha256_text "$reference|linux/amd64")
other_key=$(sha256_text "$other_reference|linux/amd64")
test "$key" != "$other_key"

cache=$test_root/cache/$key
mkdir -p "$cache" "$test_root/empty"
tar -cf "$cache/rootfs.tar" -C "$test_root/empty" .
rootfs_sha=$(sha256_file "$cache/rootfs.tar")
cat > "$cache/identity" <<EOF
FORMAT_VERSION=1
IMAGE_REF=$reference
PLATFORM=linux/amd64
ROOT_DIGEST=sha256:9db2934886712d877c932e07249c75a59e6ca27adfbccc05261987a17bef2d63
MANIFEST_DIGEST=sha256:9db2934886712d877c932e07249c75a59e6ca27adfbccc05261987a17bef2d63
CONFIG_DIGEST=sha256:0000000000000000000000000000000000000000000000000000000000000000
ROOTFS_SHA256=$rootfs_sha
EOF

GINUX_OCI_OUTPUT=human GINUX_OCI_RESULT_FILE=$test_root/result \
    ./scripts/ensure-oci-image.sh "$reference" "$test_root/cache" "$test_root/work" \
    > "$test_root/output"
grep -Fq 'Cache: hit' "$test_root/output"
test "$(sed -n '1p' "$test_root/result")" = "$cache/rootfs.tar"

if ./scripts/ensure-oci-image.sh docker.io/library/alpine:3.22 \
    "$test_root/cache" "$test_root/work" >/dev/null 2>&1; then
    printf '%s\n' 'mutable OCI tag unexpectedly accepted' >&2
    exit 1
fi
if ./scripts/ensure-oci-image.sh docker.io/library/alpine@sha256:abcd \
    "$test_root/cache" "$test_root/work" >/dev/null 2>&1; then
    printf '%s\n' 'short OCI digest unexpectedly accepted' >&2
    exit 1
fi

printf '%s\n' 'test-generic-oci: PASS'
