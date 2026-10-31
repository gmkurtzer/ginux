#!/usr/bin/env bash

set -euo pipefail

root=$(git rev-parse --show-toplevel)
cpu="$root/kernel/src/arch/x86_64/cpu.rs"
architecture="$root/kernel/src/arch/x86_64/cpu/architecture.rs"
address_space="$root/kernel/src/address_space.rs"

fail() {
    printf 'privilege-entry check: %s\n' "$*" >&2
    exit 1
}

entry=$(rg -n -m1 '^ginux_linux_syscall_entry:' "$cpu" | cut -d: -f1)
switch=$(awk -v start="$entry" 'NR > start && /mov rsp, gs:\[\{entry_return_stack\}\]/ { print NR; exit }' "$cpu")
[[ -n "$switch" ]] || fail 'missing GS-based trusted stack switch'

if awk -v start="$entry" -v stop="$switch" \
    'NR > start && NR < stop && /^[[:space:]]*(push|call)[[:space:]]/ { exit 0 } END { exit 1 }' \
    "$cpu"; then
    fail 'SYSCALL entry uses the userspace stack before switching to trusted storage'
fi

for required in \
    'ginux_nmi_entry:' \
    'lock inc byte ptr [rip + {entry_nmi_probe_hits}]' \
    'jne ginux_invalid_user_return'; do
    rg -Fq "$required" "$cpu" || fail "missing required invariant: $required"
done

for required in \
    'write_msr(GS_BASE_MSR, entry_state as u64)' \
    '2 => NMI_IST' \
    '8 => DOUBLE_FAULT_IST' \
    '18 => MACHINE_CHECK_IST' \
    'nmi_guard_page()' \
    'nmi_upper_guard_page()' \
    'machine_check_guard_page()' \
    'machine_check_upper_guard_page()'; do
    rg -Fq "$required" "$architecture" || fail "missing required invariant: $required"
done

stages=$(rg -F 'lock btr qword ptr [rip + {entry_nmi_probe_mask}]' "$cpu" | wc -l | tr -d ' ')
[[ "$stages" == 6 ]] || fail "expected six syscall transition NMI probes, found $stages"

for required in \
    'current_user_entry_context_is_valid' \
    'hostile_entry_contexts_rejected'; do
    rg -Fq "$required" "$address_space" || fail "missing hostile-entry invariant: $required"
done

printf '%s\n' 'Privilege-entry architecture check passed'
