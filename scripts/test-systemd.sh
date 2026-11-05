#!/bin/sh
set -eu

for run in cold warm repeat; do
    GINUX_GATE9_DNF=off GINUX_GATE9_NETWORK=off ./scripts/probe-systemd.sh \
        >"build/gate9-systemd-$run-driver.log"
    cp build/gate9-systemd-baseline.log "build/gate9-systemd-$run.log"
    grep -Fq 'GINUX_NATIVE_CHECK: PASS' "build/gate9-systemd-$run.log"
    grep -Fq 'GINUX_MEASURE: boot-to-login-ms=' "build/gate9-systemd-$run.log"
    printf 'systemd %s boot: PASS\n' "$run"
done

./scripts/test-systemd-failure.sh
printf '%s\n' 'test-systemd: PASS (cold, warm, repeat, offline, failure, shutdown)'
