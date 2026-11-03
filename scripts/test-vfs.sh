#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh
. ./scripts/oci-common.sh

rootfs=$GINUS_OCI_CACHE_DIR/$GINUS_OCI_ROOTFS_LAYER
index=$GINUS_OCI_CACHE_DIR/$GINUS_OCI_ROOTFS_INDEX
vfs_log=build/gate5-vfs.log
failure_log=build/gate5-vfs-failure.log

./scripts/check-rocky-host.sh
rocky_scp target/x86_64-unknown-linux-gnu/release/ginus \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginus"
rocky_scp config/domain-gate5.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate5.toml"
rocky_scp "$rootfs" \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginus-oci-rootfs.tar"
rocky_scp "$index" \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginus-oci-rootfs.index"

rocky_ssh 'chmod 0755 /var/tmp/ginus; chmod 0444 /var/tmp/ginus-oci-rootfs.tar /var/tmp/ginus-oci-rootfs.index; /var/tmp/ginus vfs /var/tmp/gate5.toml /var/tmp/ginus-oci-rootfs.tar /var/tmp/ginus-oci-rootfs.index' > "$vfs_log" 2>&1
grep -Fqx "GINUS_VFS: root=docker.io/rockylinux/rockylinux@$GINUS_OCI_MANIFEST_DIGEST" "$vfs_log"
grep -Fqx 'GINUS_VFS: base=/etc/os-release identity=Rocky-Linux' "$vfs_log"
grep -Fqx 'GINUS_VFS: base=/usr/bin/bash format=ELF64-x86_64' "$vfs_log"
grep -Fqx 'GINUS_VFS: overlay=/ginus-overlay-proof value=domain-local' "$vfs_log"
grep -Fqx 'GINUS_VFS: managed-host-paths=unexposed managed-host-fds=unexposed' "$vfs_log"
grep -Fqx 'GINUS_VFS: teardown overlay-files=0 descriptors=0' "$vfs_log"
grep -Fqx 'GINUS_VFS_CHECK: PASS' "$vfs_log"
rocky_ssh 'test ! -e /ginus-overlay-proof; test ! -e /var/tmp/ginus-overlay-proof'

rocky_ssh 'dd if=/var/tmp/ginus-oci-rootfs.index of=/var/tmp/ginus-oci-rootfs-bad.index bs=16 count=1 status=none'
if rocky_ssh '/var/tmp/ginus vfs /var/tmp/gate5.toml /var/tmp/ginus-oci-rootfs.tar /var/tmp/ginus-oci-rootfs-bad.index' > "$failure_log" 2>&1; then
    printf '%s\n' 'truncated VFS index was incorrectly accepted' >&2
    exit 1
fi
grep -Fqx 'GINUS_DOMAIN: failed reason=invalid VFS index' "$failure_log"
grep -Fqx 'GINUS_DOMAIN: cleanup resources=false managed-task=false' "$failure_log"

rocky_ssh 'rm -f /var/tmp/ginus /var/tmp/gate5.toml /var/tmp/ginus-oci-rootfs.tar /var/tmp/ginus-oci-rootfs.index /var/tmp/ginus-oci-rootfs-bad.index'
printf '%s\n' 'test-vfs: PASS'
