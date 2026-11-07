#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

log=build/gate7-process.log
./scripts/check-rocky-host.sh
rocky_scp target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux"
rocky_scp config/instance-gate7.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate7.toml"
rocky_scp target/x86_64-unknown-linux-gnu/release/process-native-fixture \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/process-native-fixture"
rocky_ssh 'chmod 0755 /var/tmp/ginux; /var/tmp/ginux process /var/tmp/gate7.toml' > "$log" 2>&1

grep -Fqx 'GINUX_PROCESS: init=1 parent=0 host-pid=unexposed' "$log"
grep -Fqx 'GINUX_PROCESS: fork-clone-exec-wait iterations=256 stale-id=denied active=1' "$log"
grep -Fqx 'GINUX_PROCESS: signal=10 blocked-wait=interrupted handler=0x4000 return=PASS' "$log"
grep -Fqx 'GINUX_PROCESS: virtual-uid=1000 virtual-gid=1000 host-credentials=unchanged regain-root=denied' "$log"
grep -Fqx 'GINUX_PROCESS: futex-wake=1 eventfd=9 epoll=readable timer-expirations=4' "$log"
grep -Fqx 'GINUX_PROCESS: shutdown tasks=0 waiters=0 events=0 timers=0 resources=released' "$log"
grep -Fqx 'GATE7_PROCESS: PASS' "$log"

rocky_ssh 'set -eu; chmod 0755 /var/tmp/process-native-fixture; uid=$(id -u); gid=$(id -g); /var/tmp/ginux process-native /var/tmp/gate7.toml /var/tmp/process-native-fixture >/var/tmp/ginux-process-native.log 2>&1 & runtime=$!; attempt=0; tracee=; while test -z "$tracee"; do tracee=$(pgrep -P "$runtime" | head -n 1 || true); attempt=$((attempt + 1)); test "$attempt" -lt 100 || exit 1; test -n "$tracee" || sleep 0.05; done; awk -v uid="$uid" -v gid="$gid" "/^Uid:/{if (\$2 != uid || \$3 != uid || \$4 != uid || \$5 != uid) exit 1} /^Gid:/{if (\$2 != gid || \$3 != gid || \$4 != gid || \$5 != gid) exit 1} /^NoNewPrivs:/{if (\$2 != 1) exit 1; seen=1} END{exit !seen}" "/proc/$tracee/status"; wait "$runtime"; cat /var/tmp/ginux-process-native.log; printf "GINUX_HOST_IDENTITY: uid=%s gid=%s no-new-privs=1 retained-nonroot\n" "$uid" "$gid"' >> "$log" 2>&1
grep -Fqx 'GINUX_NATIVE_PARENT: PASS' "$log"
grep -Fqx 'GINUX_NATIVE_PROCESS: virtual-init=1 host-pid=unexposed ptrace=attached' "$log"
grep -Fqx 'GINUX_NATIVE_PROCESS: fork=mediated child=ptrace-attached pid-translation=PASS' "$log"
grep -Fqx 'GINUX_NATIVE_PROCESS: wait=translated child-status=real tasks=0 cleanup=PASS' "$log"
grep -Fqx 'GATE7_NATIVE_PROCESS: PASS' "$log"
grep -Eq '^GINUX_HOST_IDENTITY: uid=[1-9][0-9]* gid=[1-9][0-9]* no-new-privs=1 retained-nonroot$' "$log"

rocky_ssh 'rm -f /var/tmp/ginux /var/tmp/gate7.toml /var/tmp/process-native-fixture /var/tmp/ginux-process-native.log'
printf '%s\n' 'test-process: PASS'
