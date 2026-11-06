#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

printf '%s\n' 'Ginux host install: checking Rocky host'
./scripts/check-rocky-host.sh
printf '%s\n' 'Ginux host install: copying runtime'
rocky_scp target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-runtime"
rocky_scp scripts/ginux-host-controller.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-controller"
rocky_scp scripts/ginux-host-instance-helper.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-instance-helper"
rocky_scp scripts/oci-common.sh scripts/fetch-rocky-oci.sh \
    scripts/verify-rocky-oci.sh scripts/prepare-rocky-oci.sh scripts/prepare-rocky-oci-rootfs.sh \
    scripts/ensure-rocky-oci.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/"
rocky_scp config/rocky-systemd-oci.env \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/rocky-systemd-oci.env"
rocky_scp config/instance-rocky-systemd.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/rocky-systemd.toml"
printf '%s\n' 'Ginux host install: installing controller, runtime, and image tools'
rocky_ssh "sudo install -d -m 755 /usr/local/libexec/ginux-image /etc/ginux/examples /var/lib/ginux/images /var/lib/ginux/instances /var/lib/ginux/work; sudo install -m 755 /var/tmp/ginux-runtime /usr/local/libexec/ginux-runtime; sudo install -m 755 /var/tmp/ginux-controller /usr/local/bin/ginux; sudo install -m 755 /var/tmp/ginux-instance-helper /usr/local/libexec/ginux-instance-helper; sudo install -m 755 /var/tmp/fetch-rocky-oci.sh /var/tmp/verify-rocky-oci.sh /var/tmp/prepare-rocky-oci.sh /var/tmp/prepare-rocky-oci-rootfs.sh /var/tmp/ensure-rocky-oci.sh /usr/local/libexec/ginux-image/; sudo install -m 644 /var/tmp/oci-common.sh /usr/local/libexec/ginux-image/oci-common.sh; sudo install -m 644 /var/tmp/rocky-systemd-oci.env /usr/local/libexec/ginux-image/rocky-systemd-oci.env; sudo install -m 644 /var/tmp/rocky-systemd.toml /etc/ginux/examples/rocky-systemd.toml; rm -f /var/tmp/ginux-runtime /var/tmp/ginux-controller /var/tmp/ginux-instance-helper /var/tmp/oci-common.sh /var/tmp/fetch-rocky-oci.sh /var/tmp/verify-rocky-oci.sh /var/tmp/prepare-rocky-oci.sh /var/tmp/prepare-rocky-oci-rootfs.sh /var/tmp/ensure-rocky-oci.sh /var/tmp/rocky-systemd-oci.env /var/tmp/rocky-systemd.toml; command -v ginux >/dev/null"

printf '%s\n' 'GINUX_HOST_INSTALL: PASS (runtime, controller, image tools; host state preserved)'
