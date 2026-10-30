#!/usr/bin/env bash

set -euo pipefail

root=$(git rev-parse --show-toplevel)
cpu="$root/kernel/src/arch/x86_64/cpu.rs"

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
    'write_msr(GS_BASE_MSR, entry_state as u64)' \
    '2 => NMI_IST' \
    '8 => DOUBLE_FAULT_IST' \
    '18 => MACHINE_CHECK_IST' \
    'nmi_guard_page()' \
    'nmi_upper_guard_page()' \
    'machine_check_guard_page()' \
    'machine_check_upper_guard_page()' \
    'jne ginux_invalid_user_return'; do
    rg -Fq "$required" "$cpu" || fail "missing required invariant: $required"
done

printf '%s\n' 'Privilege-entry architecture check passed'
