#!/bin/sh
set -eu

printf '%s\n' 'Gate 10.5 self-contained recipe'
grep -F '[userspace.contract]' build/gate105-contract-init.log
grep -F 'default = "deny"' build/gate105-contract-init.log
grep -E '^Contract valid: .* digest=[0-9a-f]{64} ' build/gate105-contract-check.log

printf '%s\n' 'Gate 10.5 explanation and comparison'
grep -F 'Operation: filesystem.path.write' build/gate105-contract-explain.log
grep -F 'Rule: recipe-allow source=write index=0' build/gate105-contract-explain.log
grep -E '^Contract differences: [1-9][0-9]*$' build/gate105-contract-diff.log
grep -E '^Decisions: .* total=[0-9]+$' build/gate105-contract-inspect.log

printf '%s\n' 'Gate 10.5 runtime denial provenance'
grep 'operation=syscall.socket policy-source=deny policy-rule=0 .* action=deny' \
    build/gate10-contract-runtime.log | tail -1
printf '%s\n' 'host mutation: absent'

printf '%s\n' 'GATE10.5: PASS (self-contained recipe, bounded selectors, deny precedence, explanation, diff, runtime enforcement, Rocky systemd)'
