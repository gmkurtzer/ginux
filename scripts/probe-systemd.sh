#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh
domain_root=/var/lib/ginux/domains/gate9/root
log=build/gate9-systemd-baseline.log
measurements=build/gate9-systemd-measurements.txt
gate9_dnf=${GINUX_GATE9_DNF:-on}
gate9_network=${GINUX_GATE9_NETWORK:-on}
GINUX_GATE9_DNF=$gate9_dnf GINUX_GATE9_NETWORK=$gate9_network \
    ./scripts/stage-systemd-domain.sh >/dev/null

rm -f "$measurements"
set +e
./scripts/expect-systemd-login.exp \
    "$ROCKY_KEY" "$ROCKY_SSH_PORT" "$ROCKY_SSH_USER" "$domain_root" "$log" "$measurements" \
    >/dev/null
status=$?
set -e

rocky_ssh "sudo cat /var/tmp/ginux-gate9-audit.log" >>"$log" 2>/dev/null || true

printf 'systemd baseline exit: %s\n' "$status"
tail -n 40 "$log"
./scripts/cleanup-systemd-domain.sh >/dev/null

test "$status" -eq 0
grep -Fq 'Reached target multi-user.target - Multi-User System.' "$log"
grep -Fq 'GINUX_AUTHENTICATED' "$log"
grep -Fq 'GINUX_LOCKDOWN_HOST_PATHS: BLOCKED' "$log"
grep -Fq 'GINUX_LOCKDOWN_DEVICES: BLOCKED' "$log"
grep -Fq 'GINUX_LOCKDOWN_MOUNT: BLOCKED' "$log"
grep -Fq 'GINUX_LOCKDOWN_NAMESPACE: BLOCKED' "$log"
if test "$gate9_network" = on; then
    grep -Fq 'GINUX_NETWORK_DNS_HTTPS: PASS' "$log"
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
grep -Fq 'translate surface=virtual-poweroff result=domain-shutdown' "$log"
grep -Fq 'GINUX_NATIVE_CHECK: PASS' "$log"
grep -Fq 'GINUX_MEASURE: boot-to-login-ms=' "$measurements"
grep -Fq 'GINUX_MEASURE: total-runtime-ms=' "$measurements"
printf '%s\n' 'systemd login baseline: PASS (authenticated managed shell and controlled shutdown)'
