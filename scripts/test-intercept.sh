#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

log=build/gate1-intercept.log
./scripts/check-rocky-host.sh
rocky_scp \
    target/x86_64-unknown-linux-gnu/release/ginus \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginus"
rocky_scp \
    target/x86_64-unknown-linux-gnu/release/managed-payload \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginus-managed-payload"
rocky_ssh 'chmod 0755 /var/tmp/ginus /var/tmp/ginus-managed-payload; /var/tmp/ginus' > "$log" 2>&1

grep -Fq 'GINUS: host-process pid1=no' "$log"
grep -Fq 'GINUS: syscall=getpid decision=emulate result=4242' "$log"
grep -Fq 'GINUS: syscall=write decision=mediate host-write=not-executed' "$log"
grep -Fq 'GINUS: syscall=uname decision=deny errno=EPERM' "$log"
grep -Fq 'GINUS: direct-host-syscall bypass=not-observed' "$log"
grep -Fq 'GATE1: PASS' "$log"
if grep -Fq 'UNMEDIATED_PAYLOAD_WRITE' "$log"; then
    printf '%s\n' 'interception failure: managed write reached the host' >&2
    exit 1
fi
rocky_ssh 'test "$(cat /proc/1/comm)" = systemd; systemctl is-active --quiet multi-user.target'
printf '%s\n' 'HOST: healthy after Ginus exit' >> "$log"
printf '%s\n' 'test-intercept: PASS'
