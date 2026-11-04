#!/bin/sh
set -eu

printf '%s\n' 'Gate 1 demonstration: Rocky Linux 10 boots normally under QEMU TCG.'
printf '%s\n' 'Expected: host systemd is PID 1; Ginux-us runs as an ordinary process;'
printf '%s\n' '          managed syscalls are emulated, mediated, or denied;'
printf '%s\n' '          and the Rocky host remains healthy after Ginux-us exits.'
./scripts/check-rocky-host.sh
./scripts/test-intercept.sh
grep -E '^(GINUX|GATE1):' build/gate1-intercept.log
grep -F 'HOST: healthy after Ginux-us exit' build/gate1-intercept.log
./scripts/bench-baseline.sh
printf '%s\n' 'cleanup: Rocky VM will shut down and no background process will remain'
