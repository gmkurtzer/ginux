#!/bin/sh

GINUX_OCI_CONFIG_FILE=${GINUX_OCI_CONFIG_FILE:-./config/rocky-domain-oci.env}
. "$GINUX_OCI_CONFIG_FILE"

GINUX_OCI_CACHE_DIR=${GINUX_OCI_CACHE_DIR_OVERRIDE:-$GINUX_OCI_CACHE_DIR}
GINUX_OCI_BLOBS_DIR=$GINUX_OCI_CACHE_DIR/blobs/sha256
GINUX_OCI_WORK_DIR=build/oci-work

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
    actual=$(shasum -a 256 "$path" | awk '{print $1}')
    test "sha256:$actual" = "$digest"
}
