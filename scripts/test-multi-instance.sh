#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

GINUX_OCI_CONFIG_FILE=./config/rocky-systemd-oci.env
export GINUX_OCI_CONFIG_FILE
. ./scripts/oci-common.sh
rootfs=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_LAYER

./scripts/check-rocky-host.sh
rocky_scp target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-gate17-runtime"
rocky_scp scripts/ginux-host-instance-helper.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-gate17-helper"
rocky_scp scripts/ginux-host-controller.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-gate17-controller"
rocky_scp scripts/gate17-instance-check.py \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-gate17-check"
rocky_scp scripts/gate17-multi-instance-guest.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-gate17-guest"
rocky_scp config/instance-rocky-systemd.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-gate17-rocky.toml"
rocky_scp "$rootfs" \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-gate17-rootfs.tar"

rocky_ssh 'chmod 0755 /var/tmp/ginux-gate17-guest /var/tmp/ginux-gate17-check; sudo /var/tmp/ginux-gate17-guest'

printf '%s\n' 'test-multi-instance: PASS (parallel Rocky systemd, overlays, virtual kernel, contracts, lifecycle, shared networking)'
