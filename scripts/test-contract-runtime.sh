#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh
. ./scripts/oci-common.sh

rootfs=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_LAYER
instance_root=/var/lib/ginux/instances/gate10/root
log=build/gate10-contract-runtime.log
selective_log=build/gate11-selective-deny.log

./scripts/check-rocky-host.sh
rocky_scp target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux"
rocky_scp config/instance-gate10-deny-socket.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate10.toml"
rocky_scp config/instance-gate11-selective-deny.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate11-selective.toml"
rocky_scp "$rootfs" \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-oci-rootfs.tar"

rocky_ssh "set -eu; sudo rm -rf /var/lib/ginux/instances/gate10; sudo mkdir -p '$instance_root'; sudo tar -xf /var/tmp/ginux-oci-rootfs.tar -C '$instance_root'; sudo chmod -R a-w /var/lib/ginux/instances/gate10"
rocky_ssh "sudo /var/tmp/ginux native /var/tmp/gate10.toml '$instance_root' probe" >"$log" 2>&1

# The managed stdout marker and supervisor stderr audit can share the attached
# console.  Assert the complete marker without requiring it to own the line.
grep -Fq 'GINUX_BALANCED_MUTATION: BLOCKED' "$log"
grep -Fq 'GINUX_BALANCED_NETWORK: BLOCKED' "$log"
grep -Fqx 'GINUX_NATIVE: supervisor=attached dispatch=paranoid direct-safe-syscalls=0' "$log"
grep -Eq '^GINUX_NATIVE_AUDIT: sequence=[0-9]+ instance=gate10-deny-socket .*operation=syscall.socket policy-source=exact-deny policy-rule=0 phase=entry syscall=41 action=deny result=-1$' "$log"
rocky_ssh "test ! -e '$instance_root/tmp/ginux-mutation'"

rocky_ssh "sudo /var/tmp/ginux native /var/tmp/gate11-selective.toml '$instance_root' probe" >"$selective_log" 2>&1
grep -Fq 'GINUX_BALANCED_MUTATION: BLOCKED' "$selective_log"
grep -Fq 'GINUX_BALANCED_NETWORK: BLOCKED' "$selective_log"
grep -Fqx 'GINUX_NATIVE: supervisor=attached dispatch=selective direct-safe-syscalls=1' "$selective_log"
grep -Eq '^GINUX_NATIVE_AUDIT: sequence=[0-9]+ instance=gate11-selective-deny .*operation=syscall.socket policy-source=exact-deny policy-rule=0 phase=entry syscall=41 action=deny result=-1$' "$selective_log"
grep -Eq '^GINUX_NATIVE_AUDIT: sequence=[0-9]+ instance=gate11-selective-deny .*operation=syscall.getpid .*phase=entry syscall=39 action=emulate surface=virtual-identity result=4242$' "$selective_log"
rocky_ssh "test ! -e '$instance_root/tmp/ginux-mutation'"

rocky_ssh 'sudo rm -rf /var/lib/ginux/instances/gate10; rm -f /var/tmp/ginux /var/tmp/gate10.toml /var/tmp/gate11-selective.toml /var/tmp/ginux-oci-rootfs.tar'
printf '%s\n' 'test-contract-runtime: PASS (paranoid and selective denial blocked socket; virtual identity remained mediated)'
