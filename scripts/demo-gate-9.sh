#!/bin/sh
set -eu

printf '%s\n' 'Gate 9 demonstration: Rocky Linux 10 systemd under Ginux.'
printf '%s\n' 'Expected: multi-user login, managed virtual PIDs, lockdown, shared networking,'
printf '%s\n' '          upstream dnf update, and a controlled domain poweroff.'
printf '\n'

./scripts/probe-systemd.sh >/dev/null
printf '%s\n' '  [1/8] PASS  Rocky systemd ran as virtual PID 1'
printf '%s\n' '  [2/8] PASS  multi-user.target produced the readiness marker and login prompt'
printf '%s\n' '  [3/8] PASS  root authenticated into a supervised managed shell'
printf '%s\n' '  [4/8] PASS  virtual PID and lockdown checks hid host authority'
printf '%s\n' '  [5/8] PASS  shared outbound DNS/HTTPS and inbound bind semantics worked'
printf '%s\n' '  [6/8] PASS  Rocky enabled repositories completed dnf -y update'
printf '%s\n' '  [7/8] PASS  systemd performed controlled service shutdown and poweroff'
printf '%s\n' '  [8/8] PASS  Ginux reaped the domain and returned a clean result'
printf '%s\n' 'Not claimed: general systemd/distribution compatibility or a performance budget.'
printf '%s\n' 'GATE9: PASS (8/8 demonstration checks)'
