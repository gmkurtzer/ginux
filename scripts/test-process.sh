#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

log=build/gate7-process.log
./scripts/check-rocky-host.sh
rocky_scp target/x86_64-unknown-linux-gnu/release/ginus \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginus"
rocky_scp config/domain-gate7.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate7.toml"
rocky_ssh 'chmod 0755 /var/tmp/ginus; /var/tmp/ginus process /var/tmp/gate7.toml' > "$log" 2>&1

grep -Fqx 'GINUS_PROCESS: init=1 parent=0 host-pid=unexposed' "$log"
grep -Fqx 'GINUS_PROCESS: fork-clone-exec-wait iterations=256 stale-id=denied active=1' "$log"
grep -Fqx 'GINUS_PROCESS: signal=10 blocked-wait=interrupted handler=0x4000 return=PASS' "$log"
grep -Fqx 'GINUS_PROCESS: virtual-uid=1000 virtual-gid=1000 host-credentials=unchanged regain-root=denied' "$log"
grep -Fqx 'GINUS_PROCESS: futex-wake=1 eventfd=9 epoll=readable timer-expirations=4' "$log"
grep -Fqx 'GINUS_PROCESS: shutdown tasks=0 waiters=0 events=0 timers=0 resources=released' "$log"
grep -Fqx 'GATE7_PROCESS: PASS' "$log"

rocky_ssh 'rm -f /var/tmp/ginus /var/tmp/gate7.toml'
printf '%s\n' 'test-process: PASS'
