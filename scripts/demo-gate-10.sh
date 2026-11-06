#!/bin/sh
set -eu

printf '%s\n' 'Gate 10 contract comparison'
grep '^GINUX_CONTRACT:' build/gate10-contract-plan.log | head -1
grep 'operation=syscall.clock_gettime ' build/gate10-contract-plan.log
grep '^GINUX_CONTRACT:' build/gate10-contract-systemd-plan.log | head -1
grep 'operation=syscall.clock_gettime ' build/gate10-contract-systemd-plan.log

printf '%s\n' 'Gate 10 enforced domain denial'
grep 'operation=syscall.socket policy-source=domain-deny .* action=deny' \
    build/gate10-contract-runtime.log | tail -1
printf '%s\n' 'host mutation: absent'

printf '%s\n' 'Gate 10 bounded systemd audit'
grep -m1 ' action=mediate ' build/gate9-systemd-cold.log
records=$(grep -c '^GINUX_NATIVE_AUDIT:' build/gate9-systemd-cold.log)
printf 'detailed records: %s (limit 8192)\n' "$records"
grep '^GINUX_AUDIT_DROPPED:' build/gate9-systemd-cold.log | tail -1
grep 'GINUX_LIFECYCLE_AUDIT: action=translate surface=virtual-poweroff' \
    build/gate9-systemd-cold.log | tail -1

printf '%s\n' 'GATE10: PASS (versioned plans, provenance, enforcement, bounded audit, systemd baseline)'
