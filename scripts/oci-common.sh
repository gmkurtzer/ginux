#!/bin/sh

GINUX_OCI_CONFIG_FILE=${GINUX_OCI_CONFIG_FILE:-./config/rocky-instance-oci.env}
. "$GINUX_OCI_CONFIG_FILE"

GINUX_OCI_CACHE_DIR=${GINUX_OCI_CACHE_DIR_OVERRIDE:-$GINUX_OCI_CACHE_DIR}
GINUX_OCI_BLOBS_DIR=$GINUX_OCI_CACHE_DIR/blobs/sha256
GINUX_OCI_WORK_DIR=${GINUX_OCI_WORK_DIR_OVERRIDE:-build/oci-work}

oci_sha256_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

oci_digest_hex() {
    case "$1" in
        sha256:[0-9a-f][0-9a-f]*)
            hex=${1#sha256:}
            test "${#hex}" -eq 64 || return 1
            case "$hex" in *[!0-9a-f]*) return 1 ;; esac
            printf '%s\n' "$hex"
            ;;
        *) return 1 ;;
    esac
}

oci_blob_path() {
    hex=$(oci_digest_hex "$1") || return 1
    printf '%s/%s\n' "$GINUX_OCI_BLOBS_DIR" "$hex"
}

oci_verify_blob() {
    digest=$1
    path=$(oci_blob_path "$digest") || return 1
    test -f "$path" || return 1
    actual=$(oci_sha256_file "$path")
    test "sha256:$actual" = "$digest"
}
