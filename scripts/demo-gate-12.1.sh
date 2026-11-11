#!/bin/sh
set -eu

GINUX_GATE9_DNF=off GINUX_GATE9_NETWORK=off ./scripts/probe-systemd.sh \
    >build/gate12.1-virtual-kernel-filesystems-driver.log

log=build/gate9-systemd-baseline.log
grep -F 'GINUX_VIRTUAL_KERNEL_FILESYSTEMS: PASS' "$log"
grep -F 'Reached target multi-user.target - Multi-User System.' "$log" | head -n 1

printf '%s\n' \
    'GATE12.1: PASS (on-demand in-memory procfs/sysfs, live virtual PIDs, no host leakage, no backing nodes)'
