#!/bin/sh
set -eu

GINUX_OCI_CONFIG_FILE=./config/rocky-systemd-oci.env
export GINUX_OCI_CONFIG_FILE
. ./scripts/oci-common.sh
rootfs=$GINUX_OCI_CACHE_DIR/$GINUX_OCI_ROOTFS_LAYER

for run in cold warm repeat; do
    GINUX_GATE9_DNF=off GINUX_GATE9_NETWORK=off ./scripts/probe-systemd.sh \
        >"build/gate9-systemd-$run-driver.log"
    cp build/gate9-systemd-baseline.log "build/gate9-systemd-$run.log"
    cp build/gate9-systemd-measurements.txt "build/gate9-systemd-$run-measurements.txt"
    grep -Fq 'GINUX_NATIVE_CHECK: PASS' "build/gate9-systemd-$run.log"
    grep -Fq 'GINUX_MEASURE: boot-to-login-ms=' "build/gate9-systemd-$run-measurements.txt"
    printf 'systemd %s boot: PASS\n' "$run"
done

base_hash_before=$(shasum -a 256 "$rootfs")
GINUX_GATE9_DNF=failure GINUX_GATE9_NETWORK=off ./scripts/probe-systemd.sh \
    >build/gate9-systemd-dnf-failure-driver.log
cp build/gate9-systemd-baseline.log build/gate9-systemd-dnf-failure.log
base_hash_after=$(shasum -a 256 "$rootfs")
test "$base_hash_before" = "$base_hash_after"
grep -Fq 'GINUX_DNF_UPDATE: FAILURE-PASS' build/gate9-systemd-dnf-failure.log
grep -Fq 'GINUX_NATIVE_CHECK: PASS' build/gate9-systemd-dnf-failure.log
printf '%s\n' 'systemd dnf failure: PASS (package set unchanged, base immutable)'

./scripts/test-systemd-failure.sh
printf '%s\n' 'test-systemd: PASS (cold, warm, repeat, offline, dnf failure, startup failure, shutdown)'
