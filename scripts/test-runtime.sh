#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

log=build/gate3-runtime.log
malformed_log=build/gate3-malformed.log
./scripts/check-rocky-host.sh
rocky_scp \
    target/x86_64-unknown-linux-gnu/release/ginus \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginus"
rocky_scp \
    target/x86_64-unknown-linux-gnu/release/runtime-fixture \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginus-runtime-fixture"
rocky_ssh 'chmod 0755 /var/tmp/ginus /var/tmp/ginus-runtime-fixture; /var/tmp/ginus runtime /var/tmp/ginus-runtime-fixture' > "$log" 2>&1

grep -Fqx 'GINUS_RUNTIME: backend=x86_64-interpreter' "$log"
grep -Fqx 'GINUS_RUNTIME: elf=loaded class=ELF64 machine=x86_64 type=static' "$log"
grep -Fqx 'GINUS_RUNTIME: memory=ginus-owned host-pointers=unreachable protections=enforced' "$log"
grep -Fqx 'GINUS_RUNTIME: stack=linux-x86_64 argc=2 envc=1' "$log"
grep -Fqx 'GINUS_RUNTIME: managed-output=fixture-argGINUS_GATE=3' "$log"
grep -Fqx 'GINUS_AUDIT: sequence=1 syscall=1 action=mediate result=11' "$log"
grep -Fqx 'GINUS_AUDIT: sequence=2 syscall=1 action=mediate result=12' "$log"
grep -Fqx 'GINUS_AUDIT: sequence=3 syscall=999 action=deny result=-38' "$log"
grep -Fqx 'GINUS_AUDIT: sequence=4 syscall=39 action=emulate result=4242' "$log"
grep -Fqx 'GINUS_AUDIT: sequence=5 syscall=60 action=emulate result=0' "$log"
grep -Fqx 'GINUS_RUNTIME: direct-host-syscalls=impossible interpreted-syscall=internal-event' "$log"
grep -Fqx 'GATE3: PASS' "$log"

if rocky_ssh 'dd if=/var/tmp/ginus-runtime-fixture of=/var/tmp/ginus-runtime-malformed bs=16 count=1 status=none; /var/tmp/ginus runtime /var/tmp/ginus-runtime-malformed' > "$malformed_log" 2>&1; then
    printf '%s\n' 'malformed ELF was incorrectly accepted' >&2
    exit 1
fi
grep -Fqx 'GATE3: FAIL: elf rejected reason=truncated' "$malformed_log"
rocky_ssh 'rm -f /var/tmp/ginus /var/tmp/ginus-runtime-fixture /var/tmp/ginus-runtime-malformed'
printf '%s\n' 'test-runtime: PASS'
