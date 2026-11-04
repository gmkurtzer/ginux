#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

log=build/gate8-kernel-views.log
before=build/gate8-host-before.txt
after=build/gate8-host-after.txt

./scripts/check-rocky-host.sh
rocky_scp target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux"
rocky_scp config/domain-gate8.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate8.toml"

rocky_ssh 'sha256sum /proc/1/cgroup /proc/1/mountinfo; find /sys/class/block -mindepth 1 -maxdepth 1 -printf "%f\n" | sort' > "$before"
rocky_ssh 'chmod 0755 /var/tmp/ginux; /var/tmp/ginux kernel-views /var/tmp/gate8.toml' > "$log" 2>&1
rocky_ssh 'sha256sum /proc/1/cgroup /proc/1/mountinfo; find /sys/class/block -mindepth 1 -maxdepth 1 -printf "%f\n" | sort' > "$after"

grep -Fqx 'GINUX_VIEWS: proc pids=1,2 host-pid=absent mutation=denied' "$log"
grep -Fqx 'GINUX_VIEWS: sys devices=vda,net0 host-device=absent mutation=denied' "$log"
grep -Fqx 'GINUX_VIEWS: cgroup path=/system.slice pid=2 host-mutation=denied' "$log"
grep -Fqx 'GINUX_VIEWS: identity hostname=gate8-domain namespaces=virtual host-id=absent' "$log"
grep -Fqx 'GATE8_VIEWS: PASS' "$log"
cmp "$before" "$after"

rocky_ssh 'rm -f /var/tmp/ginux /var/tmp/gate8.toml'
printf '%s\n' 'test-kernel-views: PASS'
