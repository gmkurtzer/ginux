#!/bin/sh
set -eu

GINUX_GATE9_DNF=off \
GINUX_GATE9_NETWORK=on \
GINUX_GATE9_EXTERNAL_NETWORK=off \
GINUX_GATE9_CONFIG=config/domain-gate9.75-network.toml \
GINUX_GATE9_NETWORK_CHECK=scripts/gate975-network-check.py \
    ./scripts/probe-systemd.sh

log=build/gate9-systemd-baseline.log
grep -Fq 'GINUX_GATE975_NETWORK_POLICY_TRANSLATION: PASS' "$log"
grep -Fq 'GINUX_NETWORK_AUDIT: action=translate protocol=tcp internal=18080 external=18081' "$log"
grep -Fq 'GINUX_NETWORK_AUDIT: action=deny direction=ingress port=18082 result=-1' "$log"
grep -Fq 'GINUX_NETWORK_AUDIT: action=deny direction=egress port=25 result=-1' "$log"
grep -Fq 'GINUX_NETWORK_AUDIT: action=source-bind address=127.0.0.1' "$log"
printf '%s\n' 'test-gate975-network: PASS'
