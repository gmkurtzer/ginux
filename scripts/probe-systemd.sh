#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh
instance_root=/var/lib/ginux/instances/gate9/root
log=build/gate9-systemd-baseline.log
measurements=build/gate9-systemd-measurements.txt
gate9_dnf=${GINUX_GATE9_DNF:-on}
gate9_network=${GINUX_GATE9_NETWORK:-on}
gate9_external_network=${GINUX_GATE9_EXTERNAL_NETWORK:-on}
gate9_config=${GINUX_GATE9_CONFIG:-config/examples/rocky10-systemd.toml}
gate9_network_check=${GINUX_GATE9_NETWORK_CHECK:-scripts/gate9-network-check.py}
GINUX_GATE9_DNF=$gate9_dnf GINUX_GATE9_NETWORK=$gate9_network \
    GINUX_GATE9_EXTERNAL_NETWORK=$gate9_external_network \
    GINUX_GATE9_CONFIG=$gate9_config GINUX_GATE9_NETWORK_CHECK=$gate9_network_check \
    ./scripts/stage-systemd-instance.sh >/dev/null

rm -f "$measurements"
set +e
./scripts/expect-systemd-login.exp \
    "$ROCKY_KEY" "$ROCKY_SSH_PORT" "$ROCKY_SSH_USER" "$instance_root" "$log" "$measurements" \
    >/dev/null
status=$?
set -e

./scripts/check-systemd-console.sh "$log" "$gate9_dnf"
rocky_ssh "sudo cat /var/tmp/ginux-gate9-audit.log" >>"$log" 2>/dev/null || true
rocky_ssh "sudo cat /var/lib/ginux/instances/gate9/audit.log" >>"$log" 2>/dev/null || true
rocky_ssh "test -z \"\$(sudo find '$instance_root/proc' '$instance_root/sys' -mindepth 1 -print -quit)\""

printf 'systemd baseline exit: %s\n' "$status"
tail -n 40 "$log"
./scripts/cleanup-systemd-instance.sh >/dev/null

test "$status" -eq 0
grep -Fq 'Reached target multi-user.target - Multi-User System.' "$log"
grep -Fq 'GINUX_AUTHENTICATED' "$log"
grep -Fq 'GINUX_LOCKDOWN_HOST_PATHS: BLOCKED' "$log"
grep -Fq 'GINUX_LOCKDOWN_DEVICES: BLOCKED' "$log"
grep -Fq 'GINUX_LOCKDOWN_MOUNT: BLOCKED' "$log"
grep -Fq 'GINUX_LOCKDOWN_NAMESPACE: BLOCKED' "$log"
grep -Fq 'GINUX_VIRTUAL_KERNEL_FILESYSTEMS: PASS' "$log"
if test "$gate9_network" = on; then
    if test "$gate9_external_network" = on; then
        grep -Fq 'GINUX_NETWORK_DNS_HTTPS: PASS' "$log"
    else
        grep -Fq 'GINUX_NETWORK_DNS_HTTPS: SKIPPED' "$log"
    fi
    grep -Fq 'GINUX_NETWORK_BIND_ACCEPT_CONFLICT: PASS' "$log"
else
    grep -Fq 'GINUX_NETWORK_DNS_HTTPS: SKIPPED' "$log"
    grep -Fq 'GINUX_NETWORK_BIND_ACCEPT_CONFLICT: SKIPPED' "$log"
fi
if test "$gate9_dnf" = on; then
    grep -Fq 'GINUX_DNF_UPDATE: PASS' "$log"
elif test "$gate9_dnf" = failure; then
    grep -Fq 'GINUX_DNF_UPDATE: FAILURE-PASS' "$log"
else
    grep -Fq 'GINUX_DNF_UPDATE: SKIPPED' "$log"
fi
grep -Fq 'GINUX_SHELL_CHECK_DONE' "$log"
grep -Fq 'translate surface=virtual-poweroff result=instance-shutdown' "$log"
grep -Fq 'GINUX_NATIVE_CHECK: PASS' "$log"
grep -Fq 'GINUX_MEASURE: boot-to-login-ms=' "$measurements"
grep -Fq 'GINUX_MEASURE: total-runtime-ms=' "$measurements"
printf '%s\n' 'systemd login baseline: PASS (authenticated managed shell and controlled shutdown)'
