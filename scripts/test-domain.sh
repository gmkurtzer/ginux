#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

domain_log=build/gate4-domain.log
failure_log=build/gate4-domain-failure.log
lifecycle_log=build/gate4-lifecycle.log
rerun_log=build/gate4-domain-rerun.log

./scripts/check-rocky-host.sh
rocky_scp \
    target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux"
rocky_scp \
    target/x86_64-unknown-linux-gnu/release/runtime-fixture \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-runtime-fixture"
rocky_scp \
    config/domain-gate4.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate4-valid.toml"

rocky_ssh 'chmod 0755 /var/tmp/ginux /var/tmp/ginux-runtime-fixture; /var/tmp/ginux domain /var/tmp/gate4-valid.toml /var/tmp/ginux-runtime-fixture' > "$domain_log" 2>&1
grep -Fqx 'GINUX_LIFECYCLE: state=created resources=false managed-task=false' "$domain_log"
grep -Fqx 'GINUX_LIFECYCLE: state=running resources=true managed-task=true' "$domain_log"
grep -Fqx 'GINUX_DOMAIN: managed-output=fixture-argGINUX_GATE=4' "$domain_log"
grep -Fqx 'GINUX_LIFECYCLE: state=stopped resources=false managed-task=false' "$domain_log"
grep -Fqx 'GINUX_DOMAIN: cleanup resources=false managed-task=false' "$domain_log"
grep -Fqx 'GATE4: PASS' "$domain_log"

rocky_ssh 'dd if=/var/tmp/ginux-runtime-fixture of=/var/tmp/ginux-runtime-malformed bs=16 count=1 status=none'
if rocky_ssh '/var/tmp/ginux domain /var/tmp/gate4-valid.toml /var/tmp/ginux-runtime-malformed' > "$failure_log" 2>&1; then
    printf '%s\n' 'malformed managed init was incorrectly accepted' >&2
    exit 1
fi
grep -Fqx 'GINUX_DOMAIN: failed reason=managed ELF rejected' "$failure_log"
grep -Fqx 'GINUX_LIFECYCLE: state=failed resources=false managed-task=false' "$failure_log"
grep -Fqx 'GINUX_DOMAIN: cleanup resources=false managed-task=false' "$failure_log"

rocky_ssh 'set -eu; /var/tmp/ginux lifecycle /var/tmp/gate4-valid.toml > /var/tmp/gate4-lifecycle.log 2>&1 & ginux_pid=$!; tries=0; while ! grep -q "state=running helper-pid=" /var/tmp/gate4-lifecycle.log; do tries=$((tries + 1)); test "$tries" -lt 100; sleep 0.05; done; helper_pid=$(sed -n "s/.*helper-pid=\([0-9][0-9]*\).*/\1/p" /var/tmp/gate4-lifecycle.log); kill -TERM "$ginux_pid"; wait "$ginux_pid"; test -n "$helper_pid"; if kill -0 "$helper_pid" 2>/dev/null; then echo "lifecycle helper $helper_pid survived" >&2; exit 1; fi; cat /var/tmp/gate4-lifecycle.log' > "$lifecycle_log" 2>&1
grep -Eq '^GINUX_LIFECYCLE: state=running helper-pid=[0-9]+ resources=true$' "$lifecycle_log"
grep -Fqx 'GINUX_LIFECYCLE: signal=SIGTERM decision=stop' "$lifecycle_log"
grep -Fqx 'GINUX_LIFECYCLE: helper=reaped zombies=0' "$lifecycle_log"
grep -Fqx 'GINUX_LIFECYCLE: state=stopped resources=false managed-task=false' "$lifecycle_log"
grep -Fqx 'GATE4_LIFECYCLE: PASS' "$lifecycle_log"

# Reusable after the failure and stop cases: the last run must pass too.
rocky_ssh '/var/tmp/ginux domain /var/tmp/gate4-valid.toml /var/tmp/ginux-runtime-fixture' > "$rerun_log" 2>&1
grep -Fqx 'GATE4: PASS' "$rerun_log"
rocky_ssh 'rm -f /var/tmp/ginux /var/tmp/ginux-runtime-fixture /var/tmp/ginux-runtime-malformed /var/tmp/gate4-valid.toml /var/tmp/gate4-lifecycle.log'
printf '%s\n' 'test-domain: PASS'
