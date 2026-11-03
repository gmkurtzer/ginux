#!/bin/sh
set -eu

log=build/gate1-intercept.log
./scripts/run-qemu.sh > "$log" 2>&1

grep -Fq 'GINUS: syscall=getpid decision=emulate result=4242' "$log"
grep -Fq 'GINUS: syscall=write decision=mediate host-write=not-executed' "$log"
grep -Fq 'GINUS: syscall=uname decision=deny errno=EPERM' "$log"
grep -Fq 'GINUS: direct-host-syscall bypass=not-observed' "$log"
grep -Fq 'GATE1: PASS' "$log"
if grep -Fq 'UNMEDIATED_PAYLOAD_WRITE' "$log"; then
    printf '%s\n' 'interception failure: managed write reached the host' >&2
    exit 1
fi
printf '%s\n' 'test-intercept: PASS'
