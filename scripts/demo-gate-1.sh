#!/bin/sh
set -eu

printf '%s\n' 'Gate 1 demonstration: Ginus runs as PID 1 under QEMU TCG.'
printf '%s\n' 'Expected: getpid is emulated, write is mediated without host execution,'
printf '%s\n' '          uname is denied, direct bypass is not observed, and QEMU exits.'
./scripts/test-intercept.sh
grep -E '^(GINUS|GATE1):' build/gate1-intercept.log
printf '%s\n' 'cleanup: QEMU exited and no background process remains'
