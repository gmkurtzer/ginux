#!/bin/sh
set -eu

printf '%s\n' 'Gate 8 demonstration: Ginux-owned virtual kernel views.'
printf '%s\n' 'Expected: virtual procfs, sysfs, cgroup, mount, and identity data'
printf '%s\n' '          expose only domain objects; host inventory and mutation remain denied.'
printf '\n'

./scripts/test-kernel-views.sh >/dev/null
printf '%s\n' '  [1/7] PASS  procfs exposes only virtual PIDs 1 and 2'
printf '%s\n' '  [2/7] PASS  virtual task status, memory, uptime, and mounts are consistent'
printf '%s\n' '  [3/7] PASS  sysfs exposes only assigned vda and net0 devices'
printf '%s\n' '  [4/7] PASS  bounded cgroup v2 operations remain Ginux-owned'
printf '%s\n' '  [5/7] PASS  machine, boot, hostname, and namespace identities are virtual'
printf '%s\n' '  [6/7] PASS  host-only PID/device lookups and unsupported mutations are denied'
printf '%s\n' '  [7/7] PASS  host cgroup, mount, and block-device inventories are unchanged'
printf '%s\n' 'Not claimed: full procfs/sysfs/cgroup compatibility or systemd boot (Gate 9).'
printf '%s\n' 'GATE8: PASS (7/7 demonstration checks)'
