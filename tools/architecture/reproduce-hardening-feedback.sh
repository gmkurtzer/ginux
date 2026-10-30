#!/usr/bin/env bash

set -euo pipefail

root=$(git rev-parse --show-toplevel)
cpu="$root/kernel/src/arch/x86_64/cpu.rs"
address_space="$root/kernel/src/address_space.rs"

line_of() {
    local pattern=$1
    local file=$2
    rg -n -m1 "$pattern" "$file" 2>/dev/null | cut -d: -f1 || true
}

syscall_entry=$(line_of '^ginux_linux_syscall_entry:' "$cpu")
save_rsp=$(awk -v start="$syscall_entry" 'NR >= start && /mov gs:\[\{entry_user_rsp\}\], rsp/ { print NR; exit }' "$cpu")
trusted_rsp=$(awk -v start="$syscall_entry" 'NR >= start && /mov rsp, gs:\[\{entry_return_stack\}\]/ { print NR; exit }' "$cpu")
idt_policy=$(line_of '^const fn exception_ist' "$cpu")
copy_validate=$(line_of 'if !user_range_has_permissions\(root, address, end, 0\)' "$address_space")
copy_access=$(awk -v start="$copy_validate" 'NR >= start && /read_volatile\(\)/ { print NR; exit }' "$address_space")
trusted_walk=$(line_of '^fn current_linux_leaf_trusted' "$address_space")
trusted_leaf_check=$(line_of 'value & \(PRESENT \| USER \| WRITABLE\)' "$address_space")

printf 'claim\tdisposition\tevidence\n'
first_stack_use=$(awk -v start="$syscall_entry" -v stop="$trusted_rsp" 'NR > start && NR < stop && /^[[:space:]]*(push|call)[[:space:]]/ { print NR; exit }' "$cpu")
if [[ -n "$save_rsp" && -n "$trusted_rsp" && "$save_rsp" -lt "$trusted_rsp" && -z "$first_stack_use" ]]; then
    printf 'SYSCALL entry initially uses user RSP\tresolved-or-not-reproduced\t%s:%s switches to trusted GS-based entry storage at line %s before stack use\n' "$cpu" "$save_rsp" "$trusted_rsp"
else
    printf 'SYSCALL entry initially uses user RSP\tconfirmed\tentry ordering still permits stack use before a trusted switch\n'
fi

if [[ -n "$idt_policy" ]] && rg -q '2 => NMI_IST' "$cpu" && rg -q '18 => MACHINE_CHECK_IST' "$cpu"; then
    printf 'NMI lacks a dedicated IST\tresolved-or-not-reproduced\t%s:%s assigns the NMI IST explicitly\n' "$cpu" "$idt_policy"
    printf 'machine check lacks a dedicated IST\tresolved-or-not-reproduced\t%s:%s assigns the machine-check IST explicitly\n' "$cpu" "$idt_policy"
else
    printf 'NMI lacks a dedicated IST\tconfirmed\tIDT policy has no explicit NMI IST\n'
    printf 'machine check lacks a dedicated IST\tconfirmed\tIDT policy has no explicit machine-check IST\n'
fi

if [[ -n "$copy_validate" && -n "$copy_access" && "$copy_validate" -lt "$copy_access" ]]; then
    printf 'user-copy faults are not recoverable\tconfirmed\t%s:%s validates before a direct volatile access at line %s; no fixup path exists\n' "$address_space" "$copy_validate" "$copy_access"
else
    printf 'user-copy faults are not recoverable\tresolved-or-not-reproduced\tcopy path changed\n'
fi

if [[ -n "$trusted_walk" && -n "$trusted_leaf_check" ]]; then
    printf 'software bookkeeping overrides a failed hardware walk\trejected\t%s:%s walks active page tables and the fast copy checks leaf presence/permissions at line %s\n' "$address_space" "$trusted_walk" "$trusted_leaf_check"
else
    printf 'software bookkeeping overrides a failed hardware walk\tpartial\tfast-path proof requires manual review\n'
fi
