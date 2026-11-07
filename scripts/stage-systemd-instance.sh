#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh
GINUX_OCI_CONFIG_FILE=./config/rocky-systemd-oci.env
export GINUX_OCI_CONFIG_FILE
. ./scripts/oci-common.sh

rootfs=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_LAYER
instance_root=/var/lib/ginux/instances/gate9/root
gate9_dnf=${GINUX_GATE9_DNF:-on}
gate9_network=${GINUX_GATE9_NETWORK:-on}
gate9_external_network=${GINUX_GATE9_EXTERNAL_NETWORK:-on}
gate9_config=${GINUX_GATE9_CONFIG:-config/instance-gate9.toml}
gate9_network_check=${GINUX_GATE9_NETWORK_CHECK:-scripts/gate9-network-check.py}

case $gate9_dnf in
    on|off|failure) ;;
    *) printf '%s\n' 'GINUX_GATE9_DNF must be on, off, or failure' >&2; exit 64 ;;
esac
case $gate9_network in
    on|off) ;;
    *) printf '%s\n' 'GINUX_GATE9_NETWORK must be on or off' >&2; exit 64 ;;
esac
case $gate9_external_network in
    on|off) ;;
    *) printf '%s\n' 'GINUX_GATE9_EXTERNAL_NETWORK must be on or off' >&2; exit 64 ;;
esac

./scripts/check-rocky-host.sh
rocky_scp target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux"
rocky_scp "$gate9_config" \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate9.toml"
rocky_scp "$rootfs" \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-oci-rootfs.tar"
rocky_scp scripts/gate9-shell-check.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-gate9-check"
rocky_scp "$gate9_network_check" \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-gate9-network-check"
rocky_scp scripts/ginux-host-instance-helper.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-host-instance-helper"

rocky_ssh "set -eu; sudo rm -rf /var/lib/ginux/instances/gate9; sudo mkdir -p '$instance_root'; sudo tar -xf /var/tmp/ginux-oci-rootfs.tar -C '$instance_root'; sudo install -m 755 /var/tmp/ginux-host-instance-helper /var/tmp/ginux-host-instance-helper-installed; sudo /var/tmp/ginux-host-instance-helper-installed prepare-root '$instance_root'; sudo install -m 755 /var/tmp/ginux-gate9-check '$instance_root/usr/local/sbin/ginux-gate9-check'; sudo install -m 755 /var/tmp/ginux-gate9-network-check '$instance_root/usr/local/sbin/ginux-gate9-network-check'; sudo chown 1000:1000 '$instance_root/usr/local/sbin/ginux-gate9-check' '$instance_root/usr/local/sbin/ginux-gate9-network-check'; test -x '$instance_root/sbin/init'; chmod 0755 /var/tmp/ginux"

if test "$gate9_dnf" = off; then
    rocky_ssh "sudo touch '$instance_root/run/ginux-skip-dnf'; sudo chown 1000:1000 '$instance_root/run/ginux-skip-dnf'"
elif test "$gate9_dnf" = failure; then
    rocky_ssh "sudo touch '$instance_root/run/ginux-fail-dnf'; sudo chown 1000:1000 '$instance_root/run/ginux-fail-dnf'"
fi
if test "$gate9_network" = off; then
    rocky_ssh "sudo touch '$instance_root/run/ginux-skip-network'; sudo chown 1000:1000 '$instance_root/run/ginux-skip-network'"
fi
if test "$gate9_external_network" = off; then
    rocky_ssh "sudo touch '$instance_root/run/ginux-skip-external-network'; sudo chown 1000:1000 '$instance_root/run/ginux-skip-external-network'"
fi

printf '%s\n' 'GINUX_GATE9_STAGE: PASS'
