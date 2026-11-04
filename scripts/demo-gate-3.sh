#!/bin/sh
set -eu

printf '%s\n' 'Gate 3 scope: load and interpret a bounded static Linux x86-64 ELF.'
printf '%s\n' 'Managed registers and memory are Ginus data; guest syscall instructions'
printf '%s\n' 'become internal dispatch events and never execute on the host.'
printf '%s\n' ''

./scripts/test-runtime.sh

printf '%s\n' ''
printf '%s\n' 'Successful managed execution:'
cat build/gate3-runtime.log
printf '%s\n' ''
printf '%s\n' 'Negative ELF-loading case:'
cat build/gate3-malformed.log
printf '%s\n' ''
printf '%s\n' 'Gate 3 verified:'
printf '%s\n' '  - Ginus loaded a standard static ELF64 x86-64 program from bytes'
printf '%s\n' '  - Ginus built argc/argv/environment and enforced page permissions'
printf '%s\n' '  - managed output was copied from validated Ginus memory'
printf '%s\n' '  - known syscalls were emulated/mediated and syscall 999 returned ENOSYS'
printf '%s\n' '  - malformed ELF input was rejected without running managed code'
printf '%s\n' ''
printf '%s\n' 'Not tested in Gate 3: dynamic linking, filesystems, threads, or systemd.'
printf '%s\n' 'GATE3: PASS (bounded interpreted static-program profile)'
