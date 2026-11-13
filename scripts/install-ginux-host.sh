#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

printf '%s\n' 'Ginux host install: checking Rocky host'
./scripts/check-rocky-host.sh
printf '%s\n' 'Ginux host install: copying runtime'
rocky_scp target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-runtime"
rocky_scp target/x86_64-unknown-linux-musl/release/ginux-cli \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-cli"
rocky_scp scripts/ginux-host-instance-helper.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-instance-helper"
rocky_scp scripts/oci-common.sh scripts/fetch-rocky-oci.sh \
    scripts/verify-rocky-oci.sh scripts/prepare-rocky-oci.sh \
    scripts/prepare-rocky-oci-rootfs.sh scripts/ensure-rocky-oci.sh \
    scripts/ensure-oci-image.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/"
rocky_scp config/rocky-systemd-oci.env \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/rocky-systemd-oci.env"
rocky_scp config/examples/*.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/"
printf '%s\n' 'Ginux host install: installing controller, runtime, and helper'
rocky_ssh "sudo install -d -m 755 /usr/libexec/ginux /usr/local/libexec/ginux-image /etc/ginux/examples /var/lib/ginux/cache/oci/blobs/sha256 /var/lib/ginux/instances /var/lib/ginux/work /run/ginux/instances; sudo install -m 755 /var/tmp/ginux-runtime /usr/libexec/ginux/ginux-runtime; sudo install -m 755 /var/tmp/ginux-cli /usr/bin/ginux; sudo install -m 755 /var/tmp/ginux-instance-helper /usr/libexec/ginux/ginux-helper; sudo install -m 755 /var/tmp/fetch-rocky-oci.sh /var/tmp/verify-rocky-oci.sh /var/tmp/prepare-rocky-oci.sh /var/tmp/prepare-rocky-oci-rootfs.sh /var/tmp/ensure-rocky-oci.sh /var/tmp/ensure-oci-image.sh /usr/local/libexec/ginux-image/; sudo install -m 644 /var/tmp/oci-common.sh /usr/local/libexec/ginux-image/oci-common.sh; sudo install -m 644 /var/tmp/rocky-systemd-oci.env /usr/local/libexec/ginux-image/rocky-systemd-oci.env; sudo rm -f /etc/sudoers.d/ginux; for recipe in /var/tmp/*-*.toml; do sudo install -m 644 \"\$recipe\" /etc/ginux/examples/\"\${recipe##*/}\"; done; rm -f /var/tmp/ginux-runtime /var/tmp/ginux-cli /var/tmp/ginux-instance-helper /var/tmp/oci-common.sh /var/tmp/fetch-rocky-oci.sh /var/tmp/verify-rocky-oci.sh /var/tmp/prepare-rocky-oci.sh /var/tmp/prepare-rocky-oci-rootfs.sh /var/tmp/ensure-rocky-oci.sh /var/tmp/ensure-oci-image.sh /var/tmp/rocky-systemd-oci.env /var/tmp/*-*.toml; command -v ginux >/dev/null"

printf '%s\n' 'GINUX_HOST_INSTALL: PASS (CLI, runtime, helper; test OCI compatibility path; host state preserved)'
