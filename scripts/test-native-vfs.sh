#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh
. ./scripts/oci-common.sh

rootfs=$GINUS_OCI_CACHE_DIR/$GINUS_OCI_ROOTFS_LAYER
native_log=build/gate5-native.log
domain_root=/var/lib/ginus/domains/gate5/root

./scripts/check-rocky-host.sh
rocky_scp target/x86_64-unknown-linux-gnu/release/ginus \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginus"
rocky_scp config/domain-gate5.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate5.toml"
rocky_scp "$rootfs" \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginus-oci-rootfs.tar"

rocky_ssh "set -eu; sudo rm -rf /var/lib/ginus/domains/gate5; sudo mkdir -p '$domain_root'; sudo tar -xf /var/tmp/ginus-oci-rootfs.tar -C '$domain_root'; sudo chmod -R a-w /var/lib/ginus/domains/gate5; test -x '$domain_root/usr/bin/bash'; test -x '$domain_root/usr/lib64/ld-linux-x86-64.so.2'; sha256sum '$domain_root/usr/bin/bash'" > build/gate5-bash-before.sha256

rocky_ssh "sudo /var/tmp/ginus native /var/tmp/gate5.toml '$domain_root'" > "$native_log" 2>&1
grep -Fqx 'GINUS_NATIVE: backend=ptrace-native elf-loader=host contract=gate5-bootstrap' "$native_log"
grep -Fqx 'GINUS_NATIVE: tracee-uid=1000 tracee-gid=1000 root=oci-read-only' "$native_log"
grep -Fqx 'GINUS_NATIVE_SHELL: PASS' "$native_log"
grep -Fqx 'GINUS_NATIVE: supervisor=attached direct-unsupervised-syscalls=0' "$native_log"
grep -Fqx 'GINUS_NATIVE_CHECK: PASS' "$native_log"
grep -Eq '^GINUS_NATIVE_AUDIT: sequence=[0-9]+ phase=entry syscall=[0-9]+ action=(mediate|emulate|deny)' "$native_log"
grep -Eq '^GINUS_NATIVE_AUDIT: sequence=[0-9]+ phase=entry syscall=39 action=emulate result=4242$' "$native_log"
grep -Eq '^GINUS_NATIVE_AUDIT: sequence=[0-9]+ phase=entry syscall=110 action=deny result=-38$' "$native_log"

hold_pid=$(rocky_ssh "sudo sh -c 'exec /var/tmp/ginus native /var/tmp/gate5.toml \"$domain_root\" hold > /var/tmp/ginus-hold.log 2>&1 & echo \$!'")
case "$hold_pid" in *[!0-9]*|'') printf '%s\n' 'invalid hold supervisor PID' >&2; exit 1 ;; esac
ready=0
attempt=0
while [ "$attempt" -lt 50 ]; do
    if rocky_ssh "grep -Fqx 'GINUS_NATIVE_HOLD: READY' /var/tmp/ginus-hold.log" >/dev/null 2>&1; then
        ready=1
        break
    fi
    sleep 1
    attempt=$((attempt + 1))
done
test "$ready" -eq 1
tracee_pid=$(rocky_ssh "pgrep -P '$hold_pid' -x bash")
case "$tracee_pid" in *[!0-9]*|'') printf '%s\n' 'invalid held tracee PID' >&2; exit 1 ;; esac
rocky_ssh "sudo kill -9 '$hold_pid'; attempt=0; while sudo kill -0 '$tracee_pid' 2>/dev/null; do attempt=\$((attempt + 1)); test \$attempt -lt 50; sleep 0.1; done"
if rocky_ssh "sudo kill -0 '$tracee_pid'" >/dev/null 2>&1; then
    printf '%s\n' 'tracee survived supervisor loss' >&2
    exit 1
fi

rocky_ssh "sha256sum '$domain_root/usr/bin/bash'" > build/gate5-bash-after.sha256
cmp build/gate5-bash-before.sha256 build/gate5-bash-after.sha256
rocky_ssh 'sudo rm -rf /var/lib/ginus/domains/gate5 /var/tmp/ginus-hold.log; rm -f /var/tmp/ginus /var/tmp/gate5.toml /var/tmp/ginus-oci-rootfs.tar'
printf '%s\n' 'test-native-vfs: PASS'
