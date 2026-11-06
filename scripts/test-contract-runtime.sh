#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh
. ./scripts/oci-common.sh

rootfs=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_LAYER
domain_root=/var/lib/ginux/domains/gate10/root
log=build/gate10-contract-runtime.log

./scripts/check-rocky-host.sh
rocky_scp target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux"
rocky_scp config/domain-gate10-deny-socket.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate10.toml"
rocky_scp "$rootfs" \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-oci-rootfs.tar"

rocky_ssh "set -eu; sudo rm -rf /var/lib/ginux/domains/gate10; sudo mkdir -p '$domain_root'; sudo tar -xf /var/tmp/ginux-oci-rootfs.tar -C '$domain_root'; sudo chmod -R a-w /var/lib/ginux/domains/gate10"
rocky_ssh "sudo /var/tmp/ginux native /var/tmp/gate10.toml '$domain_root' probe" >"$log" 2>&1

grep -Fqx 'GINUX_BALANCED_MUTATION: BLOCKED' "$log"
grep -Fqx 'GINUX_BALANCED_NETWORK: BLOCKED' "$log"
grep -Eq '^GINUX_NATIVE_AUDIT: sequence=[0-9]+ domain=gate10-deny-socket .*operation=syscall.socket policy-source=domain-deny phase=entry syscall=41 action=deny result=-1$' "$log"
rocky_ssh "test ! -e '$domain_root/tmp/ginux-mutation'"

rocky_ssh 'sudo rm -rf /var/lib/ginux/domains/gate10; rm -f /var/tmp/ginux /var/tmp/gate10.toml /var/tmp/ginux-oci-rootfs.tar'
printf '%s\n' 'test-contract-runtime: PASS (domain denial blocked socket and mutation had no host effect)'
