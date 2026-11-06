#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

GINUX_GATE9_DNF=off \
GINUX_GATE9_NETWORK=on \
GINUX_GATE9_EXTERNAL_NETWORK=off \
GINUX_GATE9_CONFIG=config/domain-gate9.75-network.toml \
    GINUX_GATE9_NETWORK_CHECK=scripts/gate975-network-check.py \
    ./scripts/probe-systemd.sh

rocky_scp config/domain-gate9.75-network.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate9.toml"
rocky_scp target/x86_64-unknown-linux-gnu/release/ginux \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux"
rocky_scp scripts/ginux-host-domain-helper.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-host-domain-helper-new"
rocky_ssh 'chmod 0755 /var/tmp/ginux-host-domain-helper-new; sudo install -m 755 /var/tmp/ginux-host-domain-helper-new /var/tmp/ginux-host-domain-helper-installed; sudo install -m 755 /var/tmp/ginux /usr/local/libexec/ginux-runtime; sudo mkdir -p /var/lib/ginux/work'
rocky_ssh "sudo rm -f /var/tmp/gate975-auto-source.toml; sed '/^[[:space:]]*source[[:space:]]*=/d' /var/tmp/gate9.toml >/var/tmp/gate975-auto-source.toml; sudo /var/tmp/ginux-host-domain-helper-installed resolve-network /var/tmp/gate975-auto-source.toml; sudo grep -Fq 'source = \"127.0.0.1\"' /var/tmp/gate975-auto-source.toml"
rocky_ssh "sudo rm -f /var/tmp/gate975-missing-interface.toml /var/tmp/gate975-missing-interface.log; sed -e 's/interface = \"lo\"/interface = \"ginux-missing0\"/' -e '/^[[:space:]]*source[[:space:]]*=/d' /var/tmp/gate9.toml >/var/tmp/gate975-missing-interface.toml; if sudo /var/tmp/ginux-host-domain-helper-installed resolve-network /var/tmp/gate975-missing-interface.toml >/var/tmp/gate975-missing-interface.log 2>&1; then exit 1; fi; sudo grep -Fq 'configured network attachment is unavailable: ginux-missing0' /var/tmp/gate975-missing-interface.log"

log=build/gate9-systemd-baseline.log
grep -Fq 'GINUX_GATE975_NETWORK_POLICY_TRANSLATION: PASS' "$log"
grep -Fq 'GINUX_NETWORK_AUDIT: action=translate protocol=tcp internal=18080 external=18081' "$log"
grep -Fq 'GINUX_NETWORK_AUDIT: action=deny direction=ingress port=18082 result=-1' "$log"
grep -Fq 'GINUX_NETWORK_AUDIT: action=deny direction=egress port=25 result=-1' "$log"
grep -Fq 'GINUX_NETWORK_AUDIT: action=source-bind address=127.0.0.1' "$log"
printf '%s\n' 'test-gate975-network: PASS'
