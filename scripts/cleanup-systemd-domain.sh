#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

rocky_ssh 'sudo rm -rf /var/lib/ginux/domains/gate9; rm -f /var/tmp/ginux /var/tmp/gate9.toml /var/tmp/ginux-oci-rootfs.tar /var/tmp/ginux-gate9-check /var/tmp/ginux-gate9-network-check /var/tmp/ginux-systemd-audit.log'
printf '%s\n' 'GINUX_GATE9_CLEANUP: PASS'
