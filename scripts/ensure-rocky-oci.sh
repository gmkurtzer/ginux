#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
. "$script_dir/oci-common.sh"

rootfs=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_LAYER
index=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_INDEX
identity=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_IDENTITY
rootfs_only=${GINUX_OCI_NATIVE_ROOTFS_ONLY:-0}

prepared_cache_valid() {
    test -r "$rootfs" && test -r "$identity" || return 1
    if test "$rootfs_only" != 1; then
        test -r "$index" || return 1
    fi
    # The shell assignment file is generated solely by Ginux, but compare it
    # as data rather than sourcing cache content.
    grep -Fqx "OCI_INDEX_DIGEST=$GINUX_OCI_INDEX_DIGEST" "$identity" || return 1
    grep -Fqx "OCI_MANIFEST_DIGEST=$GINUX_OCI_MANIFEST_DIGEST" "$identity" || return 1
    expected_rootfs=$(sed -n 's/^ROOTFS_SHA256=//p' "$identity")
    expected_index=$(sed -n 's/^INDEX_SHA256=//p' "$identity")
    test -n "$expected_rootfs" || return 1
    test "$(oci_sha256_file "$rootfs")" = "$expected_rootfs" || return 1
    if test "$rootfs_only" != 1; then
        test -n "$expected_index" || return 1
        test "$(oci_sha256_file "$index")" = "$expected_index" || return 1
    fi
    "$script_dir/verify-rocky-oci.sh" >/dev/null
}

if prepared_cache_valid; then
    printf 'OCI_CACHE: hit image=docker.io/%s@%s rootfs=%s\n' \
        "$GINUX_OCI_REPOSITORY" "$GINUX_OCI_MANIFEST_DIGEST" "$rootfs"
    exit 0
fi

printf 'OCI_CACHE: miss image=docker.io/%s@%s\n' \
    "$GINUX_OCI_REPOSITORY" "$GINUX_OCI_MANIFEST_DIGEST"
"$script_dir/fetch-rocky-oci.sh"
if test "$rootfs_only" = 1; then
    "$script_dir/prepare-rocky-oci-rootfs.sh"
else
    "$script_dir/prepare-rocky-oci.sh"
fi
prepared_cache_valid
printf 'OCI_CACHE: ready image=docker.io/%s@%s rootfs=%s\n' \
    "$GINUX_OCI_REPOSITORY" "$GINUX_OCI_MANIFEST_DIGEST" "$rootfs"
