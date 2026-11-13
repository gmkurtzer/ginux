#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh
. ./scripts/oci-common.sh

rootfs=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_LAYER
instance_root=/var/lib/ginux/instances/gate9-failure/root
log=build/gate9-systemd-failure.log

rocky_scp target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux"
rocky_scp config/instance-gate9-failure.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate9-failure.toml"
rocky_scp "$rootfs" \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-oci-rootfs.tar"

rocky_ssh "set -eu; sudo rm -rf /var/lib/ginux/instances/gate9-failure; sudo mkdir -p '$instance_root'; sudo tar -xf /var/tmp/ginux-oci-rootfs.tar -C '$instance_root'; sudo chown -R 1000:1000 '$instance_root'; sudo find '$instance_root' -type d -exec chmod u+w {} +"
status=$(rocky_ssh 'set +e; /var/tmp/ginux native /var/tmp/gate9-failure.toml /var/lib/ginux/instances/gate9-failure/root >/var/tmp/gate9-failure.log 2>&1; result=$?; printf "%s\n" "$result"')
rocky_ssh 'cat /var/tmp/gate9-failure.log' >"$log"

test "$status" -ne 0
grep -Fq 'GINUX_INSTANCE: failed reason=native tracee did not stop before filter install' "$log"
grep -Fq 'GINUX_INSTANCE: cleanup resources=false managed-task=false' "$log"
if grep -Fq 'GINUX_AUTHENTICATED' "$log"; then
    printf '%s\n' 'configured failure unexpectedly reached a shell' >&2
    exit 1
fi
./scripts/check-rocky-host.sh >/dev/null

rocky_ssh 'sudo rm -rf /var/lib/ginux/instances/gate9-failure; rm -f /var/tmp/ginux /var/tmp/gate9-failure.toml /var/tmp/ginux-oci-rootfs.tar /var/tmp/gate9-failure.log'
printf '%s\n' 'systemd configured failure: PASS (nonzero, diagnosed, cleaned, no fallback)'
