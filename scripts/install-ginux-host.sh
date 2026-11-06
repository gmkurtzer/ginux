#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh
GINUX_OCI_CONFIG_FILE=./config/rocky-systemd-oci.env
export GINUX_OCI_CONFIG_FILE
. ./scripts/oci-common.sh

rootfs=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_LAYER

./scripts/check-rocky-host.sh
rocky_scp target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-runtime"
rocky_scp scripts/ginux-host-controller.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-controller"
rocky_scp scripts/ginux-host-domain-helper.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-domain-helper"
rocky_scp config/domain-rocky-systemd.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/rocky-systemd.toml"
rocky_scp "$rootfs" \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-rocky-rootfs.tar"
rocky_ssh "sudo install -d -m 755 /usr/local/libexec /etc/ginux/examples /var/lib/ginux/images /var/lib/ginux/domains; sudo install -m 755 /var/tmp/ginux-runtime /usr/local/libexec/ginux-runtime; sudo install -m 755 /var/tmp/ginux-controller /usr/local/bin/ginux; sudo install -m 755 /var/tmp/ginux-domain-helper /usr/local/libexec/ginux-domain-helper; sudo install -m 644 /var/tmp/rocky-systemd.toml /etc/ginux/examples/rocky-systemd.toml; sudo install -m 644 /var/tmp/ginux-rocky-rootfs.tar /var/lib/ginux/images/rocky-10-ubi-init-amd64-rootfs.tar; rm -f /var/tmp/ginux-runtime /var/tmp/ginux-controller /var/tmp/ginux-domain-helper /var/tmp/rocky-systemd.toml /var/tmp/ginux-rocky-rootfs.tar; command -v ginux >/dev/null; test -z \"\$(sudo find /var/lib/ginux/domains -mindepth 1 -maxdepth 1 -print -quit)\""

printf '%s\n' 'GINUX_HOST_INSTALL: PASS (runtime, controller, example, image; no domains)'
