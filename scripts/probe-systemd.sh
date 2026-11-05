#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh
GINUX_OCI_CONFIG_FILE=./config/rocky-systemd-oci.env
export GINUX_OCI_CONFIG_FILE
. ./scripts/oci-common.sh

rootfs=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_LAYER
domain_root=/var/lib/ginux/domains/gate9/root
log=build/gate9-systemd-baseline.log

./scripts/check-rocky-host.sh
rocky_scp target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux"
rocky_scp config/domain-gate9.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate9.toml"
rocky_scp "$rootfs" \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-oci-rootfs.tar"

rocky_ssh "set -eu; sudo rm -rf /var/lib/ginux/domains/gate9; sudo mkdir -p '$domain_root'; sudo tar -xf /var/tmp/ginux-oci-rootfs.tar -C '$domain_root'; test -x '$domain_root/sbin/init'"

set +e
rocky_ssh "timeout 30s sudo /var/tmp/ginux native /var/tmp/gate9.toml '$domain_root' systemd" > "$log" 2>&1
status=$?
set -e

printf 'systemd baseline exit: %s\n' "$status"
tail -n 40 "$log"
rocky_ssh 'sudo rm -rf /var/lib/ginux/domains/gate9; rm -f /var/tmp/ginux /var/tmp/gate9.toml /var/tmp/ginux-oci-rootfs.tar'

test "$status" -ne 124
