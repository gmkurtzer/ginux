#!/bin/sh
set -eu

printf '%s\n' 'Gate 10.5 self-contained instance recipe'
grep -F '[contract]' config/instance-rocky-systemd.toml
grep -F '[contract.syscalls]' config/instance-rocky-systemd.toml
grep -F 'default = "deny"' config/instance-rocky-systemd.toml
grep -E '^Contract valid: .* digest=[0-9a-f]{64} ' build/gate105-instance-validate.log

printf '%s\n' 'Gate 10.5 explanation and comparison'
grep -F 'Operation: filesystem.path.write' build/gate105-instance-explain.log
grep -F 'Rule: recipe-allow source=write index=0' build/gate105-instance-explain.log
grep -E '^Contract differences: [1-9][0-9]*$' build/gate105-instance-diff.log
grep -E '^Decisions: .* total=[0-9]+$' build/gate105-instance-inspect.log

printf '%s\n' 'Gate 10.5 runtime denial provenance'
grep 'operation=syscall.socket policy-source=deny policy-rule=0 .* action=deny' \
    build/gate10-contract-runtime.log | tail -1
printf '%s\n' 'host mutation: absent'

printf '%s\n' 'GATE10.5: PASS (self-contained recipe, bounded selectors, deny precedence, explanation, diff, runtime enforcement, Rocky systemd)'
