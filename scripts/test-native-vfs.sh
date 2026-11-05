#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh
. ./scripts/oci-common.sh

rootfs=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_LAYER
native_log=build/gate5-native.log
domain_root=/var/lib/ginux/domains/gate5/root

./scripts/check-rocky-host.sh
rocky_scp target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux"
rocky_scp config/domain-gate5.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate5.toml"
rocky_scp "$rootfs" \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-oci-rootfs.tar"

rocky_ssh "set -eu; sudo rm -rf /var/lib/ginux/domains/gate5; sudo mkdir -p '$domain_root'; sudo tar -xf /var/tmp/ginux-oci-rootfs.tar -C '$domain_root'; sudo chmod -R a-w /var/lib/ginux/domains/gate5; test -x '$domain_root/usr/bin/bash'; test -x '$domain_root/usr/lib64/ld-linux-x86-64.so.2'; sha256sum '$domain_root/usr/bin/bash'" > build/gate5-bash-before.sha256

rocky_ssh "sudo /var/tmp/ginux native /var/tmp/gate5.toml '$domain_root'" > "$native_log" 2>&1
grep -Fqx 'GINUX_NATIVE: backend=ptrace-native profile=balanced elf-loader=host contract=gate5-balanced' "$native_log"
grep -Fqx 'GINUX_NATIVE: tracee-uid=1000 tracee-gid=1000 root=oci-read-only' "$native_log"
grep -Fqx 'GINUX_NATIVE_SHELL: PASS' "$native_log"
grep -Fqx 'GINUX_NATIVE: supervisor=attached direct-unsupervised-syscalls=0' "$native_log"
grep -Fqx 'GINUX_NATIVE_CHECK: PASS' "$native_log"
grep -Eq '^GINUX_NATIVE_AUDIT: sequence=[0-9]+ phase=entry syscall=[0-9]+ action=(mediate|emulate|deny)' "$native_log"
grep -Eq '^GINUX_NATIVE_AUDIT: sequence=[0-9]+ phase=entry syscall=39 action=emulate surface=virtual-identity result=4242$' "$native_log"
grep -Eq '^GINUX_NATIVE_AUDIT: sequence=[0-9]+ phase=entry syscall=110 action=emulate surface=virtual-identity result=1$' "$native_log"
grep -Eq '^GINUX_NATIVE_AUDIT: sequence=[0-9]+ phase=entry syscall=102 action=emulate surface=virtual-identity result=0$' "$native_log"
grep -Eq '^GINUX_NATIVE_AUDIT: sequence=[0-9]+ phase=entry syscall=104 action=emulate surface=virtual-identity result=0$' "$native_log"
grep -Eq '^GINUX_NATIVE_AUDIT: sequence=[0-9]+ phase=entry syscall=16 action=deny result=-38$' "$native_log"
grep -Eq '^GINUX_NATIVE_AUDIT: sequence=[0-9]+ phase=entry syscall=257 action=mediate surface=immutable-image result=pending$' "$native_log"
grep -Eq '^GINUX_NATIVE_AUDIT: sequence=[0-9]+ phase=entry syscall=1 action=mediate surface=bootstrap-console result=pending$' "$native_log"
printf '%s\n' '  native shell: PASS (Rocky loader and Bash, balanced audit surfaces)'

rocky_ssh "sudo /var/tmp/ginux native /var/tmp/gate5.toml '$domain_root' probe" > build/gate5-balanced-probe.log 2>&1
grep -Fqx 'GINUX_BALANCED_MUTATION: BLOCKED' build/gate5-balanced-probe.log
grep -Fqx 'GINUX_BALANCED_NETWORK: BLOCKED' build/gate5-balanced-probe.log
grep -Eq '^GINUX_NATIVE_AUDIT: sequence=[0-9]+ phase=entry syscall=257 action=deny result=-1 reason=invalid-arguments$' build/gate5-balanced-probe.log
grep -Eq '^GINUX_NATIVE_AUDIT: sequence=[0-9]+ phase=entry syscall=41 action=deny result=-38$' build/gate5-balanced-probe.log
rocky_ssh "test ! -e '$domain_root/tmp/ginux-mutation'"
printf '%s\n' '  sensitive probes: PASS (mutation and networking denied before effect)'

hold_pid=$(rocky_ssh "sudo sh -c 'exec /var/tmp/ginux native /var/tmp/gate5.toml \"$domain_root\" hold > /var/tmp/ginux-hold.log 2>&1 & echo \$!'")
case "$hold_pid" in *[!0-9]*|'') printf '%s\n' 'invalid hold supervisor PID' >&2; exit 1 ;; esac
ready=0
attempt=0
while [ "$attempt" -lt 50 ]; do
    if rocky_ssh "grep -Fqx 'GINUX_NATIVE_HOLD: READY' /var/tmp/ginux-hold.log" >/dev/null 2>&1; then
        ready=1
        break
    fi
    sleep 1
    attempt=$((attempt + 1))
done
test "$ready" -eq 1
tracee_pid=$(rocky_ssh "pgrep -P '$hold_pid' -x bash")
case "$tracee_pid" in *[!0-9]*|'') printf '%s\n' 'invalid held tracee PID' >&2; exit 1 ;; esac
rocky_ssh "sudo kill -9 '$hold_pid'; attempt=0; while sudo kill -0 '$tracee_pid' 2>/dev/null; do attempt=\$((attempt + 1)); test \$attempt -lt 50 || exit 1; sleep 0.1; done"
if rocky_ssh "sudo kill -0 '$tracee_pid'" >/dev/null 2>&1; then
    printf '%s\n' 'tracee survived supervisor loss' >&2
    exit 1
fi
printf '%s\n' '  supervisor loss: PASS (held tracee terminated by PTRACE_O_EXITKILL)'

stop_pid=$(rocky_ssh "sudo sh -c 'exec /var/tmp/ginux native /var/tmp/gate5.toml \"$domain_root\" hold > /var/tmp/ginux-stop.log 2>&1 & echo \$!'")
case "$stop_pid" in *[!0-9]*|'') printf '%s\n' 'invalid unexpected-stop supervisor PID' >&2; exit 1 ;; esac
ready=0
attempt=0
while [ "$attempt" -lt 50 ]; do
    if rocky_ssh "grep -Fqx 'GINUX_NATIVE_HOLD: READY' /var/tmp/ginux-stop.log" >/dev/null 2>&1; then
        ready=1
        break
    fi
    sleep 1
    attempt=$((attempt + 1))
done
test "$ready" -eq 1
stopped_tracee_pid=$(rocky_ssh "pgrep -P '$stop_pid' -x bash")
case "$stopped_tracee_pid" in *[!0-9]*|'') printf '%s\n' 'invalid unexpected-stop tracee PID' >&2; exit 1 ;; esac
rocky_ssh "sudo kill -STOP '$stopped_tracee_pid'"
attempt=0
while [ "$attempt" -lt 50 ]; do
    if rocky_ssh "grep -Fqx 'GINUX_DOMAIN: failed reason=unexpected native trace stop' /var/tmp/ginux-stop.log" >/dev/null 2>&1; then
        break
    fi
    sleep 0.1
    attempt=$((attempt + 1))
done
grep_result=$(rocky_ssh "grep -Fxc 'GINUX_DOMAIN: failed reason=unexpected native trace stop' /var/tmp/ginux-stop.log")
test "$grep_result" -eq 1
if rocky_ssh "sudo kill -0 '$stopped_tracee_pid'" >/dev/null 2>&1; then
    printf '%s\n' 'tracee survived unexpected ptrace stop' >&2
    exit 1
fi
rocky_ssh "grep -Fqx 'GINUX_DOMAIN: cleanup resources=false managed-task=false' /var/tmp/ginux-stop.log"
printf '%s\n' '  unexpected stop: PASS (tracee killed, reaped, and domain cleaned)'

rocky_ssh "sha256sum '$domain_root/usr/bin/bash'" > build/gate5-bash-after.sha256
cmp build/gate5-bash-before.sha256 build/gate5-bash-after.sha256
printf '%s\n' '  immutable Bash: PASS (digest unchanged)'
rocky_ssh 'sudo rm -rf /var/lib/ginux/domains/gate5 /var/tmp/ginux-hold.log /var/tmp/ginux-stop.log; rm -f /var/tmp/ginux /var/tmp/gate5.toml /var/tmp/ginux-oci-rootfs.tar'
printf '%s\n' 'test-native-vfs: PASS'
