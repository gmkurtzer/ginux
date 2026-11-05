#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh
. ./scripts/oci-common.sh

rootfs=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_LAYER
index=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_INDEX
vfs_log=build/gate5-vfs.log
failure_log=build/gate5-vfs-failure.log
missing_log=build/gate5-vfs-missing.log

./scripts/check-rocky-host.sh
rocky_scp target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux"
rocky_scp config/domain-gate5.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate5.toml"
rocky_scp "$rootfs" \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-oci-rootfs.tar"
rocky_scp "$index" \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-oci-rootfs.index"

rocky_ssh 'chmod 0755 /var/tmp/ginux; chmod 0444 /var/tmp/ginux-oci-rootfs.tar /var/tmp/ginux-oci-rootfs.index; /var/tmp/ginux vfs /var/tmp/gate5.toml /var/tmp/ginux-oci-rootfs.tar /var/tmp/ginux-oci-rootfs.index' > "$vfs_log" 2>&1
grep -Fqx "GINUX_VFS: root=docker.io/rockylinux/rockylinux@$GINUX_OCI_MANIFEST_DIGEST" "$vfs_log"
grep -Fqx 'GINUX_VFS: base=/etc/os-release identity=Rocky-Linux' "$vfs_log"
grep -Fqx 'GINUX_VFS: base=/usr/bin/bash format=ELF64-x86_64' "$vfs_log"
grep -Fqx 'GINUX_VFS: overlay=/ginux-overlay-proof value=domain-local' "$vfs_log"
grep -Fqx 'GINUX_VFS: managed-host-paths=unexposed managed-host-fds=unexposed' "$vfs_log"
grep -Fqx 'GINUX_VFS: teardown overlay-files=0 descriptors=0' "$vfs_log"
grep -Fqx 'GINUX_VFS_CHECK: PASS' "$vfs_log"
rocky_ssh 'test ! -e /ginux-overlay-proof; test ! -e /var/tmp/ginux-overlay-proof'

rocky_ssh 'dd if=/var/tmp/ginux-oci-rootfs.index of=/var/tmp/ginux-oci-rootfs-bad.index bs=16 count=1 status=none'
if rocky_ssh '/var/tmp/ginux vfs /var/tmp/gate5.toml /var/tmp/ginux-oci-rootfs.tar /var/tmp/ginux-oci-rootfs-bad.index' > "$failure_log" 2>&1; then
    printf '%s\n' 'truncated VFS index was incorrectly accepted' >&2
    exit 1
fi
grep -Fqx 'GINUX_DOMAIN: failed reason=invalid VFS index' "$failure_log"
grep -Fqx 'GINUX_DOMAIN: cleanup resources=false managed-task=false' "$failure_log"

rocky_ssh 'rm -f /var/tmp/ginux-oci-rootfs-missing.index'
if rocky_ssh '/var/tmp/ginux vfs /var/tmp/gate5.toml /var/tmp/ginux-oci-rootfs.tar /var/tmp/ginux-oci-rootfs-missing.index' > "$missing_log" 2>&1; then
    printf '%s\n' 'missing VFS index was incorrectly accepted' >&2
    exit 1
fi
grep -Fqx 'GINUX_DOMAIN: failed reason=VFS index open failed' "$missing_log"
grep -Fqx 'GINUX_LIFECYCLE: state=failed resources=false managed-task=false' "$missing_log"
grep -Fqx 'GINUX_DOMAIN: cleanup resources=false managed-task=false' "$missing_log"

rocky_ssh 'rm -f /var/tmp/ginux /var/tmp/gate5.toml /var/tmp/ginux-oci-rootfs.tar /var/tmp/ginux-oci-rootfs.index /var/tmp/ginux-oci-rootfs-bad.index'
printf '%s\n' 'test-vfs: PASS'
