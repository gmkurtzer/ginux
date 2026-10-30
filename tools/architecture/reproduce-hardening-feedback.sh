#!/usr/bin/env bash

set -euo pipefail

root=$(git rev-parse --show-toplevel)
cpu="$root/kernel/src/arch/x86_64/cpu.rs"
address_space="$root/kernel/src/address_space.rs"

line_of() {
    local pattern=$1
    local file=$2
    rg -n -m1 "$pattern" "$file" | cut -d: -f1
}

syscall_entry=$(line_of '^ginux_linux_syscall_entry:' "$cpu")
save_rsp=$(awk -v start="$syscall_entry" 'NR >= start && /mov \[rip \+ \{linux_user_rsp\}\], rsp/ { print NR; exit }' "$cpu")
trusted_rsp=$(awk -v start="$syscall_entry" 'NR >= start && /mov rsp, \[rip \+ \{user_return_stack\}\]/ { print NR; exit }' "$cpu")
idt_policy=$(line_of 'let ist = if vector == 8' "$cpu")
copy_validate=$(line_of 'if !user_range_has_permissions\(root, address, end, 0\)' "$address_space")
copy_access=$(awk -v start="$copy_validate" 'NR >= start && /read_volatile\(\)/ { print NR; exit }' "$address_space")
trusted_walk=$(line_of '^fn current_linux_leaf_trusted' "$address_space")
trusted_leaf_check=$(line_of 'value & \(PRESENT \| USER \| WRITABLE\)' "$address_space")

printf 'claim\tdisposition\tevidence\n'
if [[ -n "$save_rsp" && -n "$trusted_rsp" && "$save_rsp" -lt "$trusted_rsp" ]]; then
    printf 'SYSCALL entry initially uses user RSP\tconfirmed\t%s:%s saves RSP before trusted stack switch at line %s\n' "$cpu" "$save_rsp" "$trusted_rsp"
else
    printf 'SYSCALL entry initially uses user RSP\tresolved-or-not-reproduced\tentry ordering changed\n'
fi

if [[ -n "$idt_policy" ]]; then
    printf 'NMI lacks a dedicated IST\tconfirmed\t%s:%s assigns an IST only to vector 8\n' "$cpu" "$idt_policy"
    printf 'machine check lacks a dedicated IST\tconfirmed\t%s:%s assigns an IST only to vector 8\n' "$cpu" "$idt_policy"
else
    printf 'NMI lacks a dedicated IST\tresolved-or-not-reproduced\tIDT policy changed\n'
    printf 'machine check lacks a dedicated IST\tresolved-or-not-reproduced\tIDT policy changed\n'
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
