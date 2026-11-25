#!/bin/sh
set -eu

GINUX_GATE9_DNF=on GINUX_GATE9_NETWORK=on ./scripts/probe-systemd.sh
cp build/gate9-systemd-baseline.log build/gate9-systemd-live.log
grep -Fq 'GINUX_NETWORK_DNS_HTTPS: PASS' build/gate9-systemd-live.log
grep -Fq 'GINUX_NETWORK_BIND_ACCEPT_CONFLICT: PASS' build/gate9-systemd-live.log
grep -Fq 'GINUX_DNF_UPDATE: PASS' build/gate9-systemd-live.log
printf '%s\n' 'test-systemd-live: PASS (shared network and upstream dnf update)'
