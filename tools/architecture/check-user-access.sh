#!/usr/bin/env bash

set -euo pipefail

root=$(git rev-parse --show-toplevel)
cpu="$root/kernel/src/arch/x86_64/cpu.rs"
address_space="$root/kernel/src/address_space.rs"

fail() {
    printf 'user-access check: %s\n' "$*" >&2
    exit 1
}

for required in \
    'ginux_raw_copy_from_user:' \
    'ginux_copy_from_user_fault_begin:' \
    'ginux_copy_from_user_fault_end:' \
    'ginux_raw_copy_to_user:' \
    'ginux_copy_to_user_fault_begin:' \
    'ginux_copy_to_user_fault_end:' \
    'ginux_copy_from_user_fixup:' \
    'ginux_copy_to_user_fixup:' \
    'verify_user_copy_fault_fixups'; do
    rg -Fq "$required" "$cpu" || fail "missing recoverable user-copy invariant: $required"
done

for required in \
    'copy_from_current_user_partial' \
    'copy_to_current_user_partial'; do
    rg -Fq "$required" "$address_space" || fail "missing audited user-copy API: $required"
done

# The SMAP-opening closure remains available only for page-table construction
# and address-space verification. Syscall implementation code must use the
# fault-recovering raw or public copy APIs instead of opening SMAP directly.
cpu_smap_sites=$(rg -F 'with_supervisor_user_access' "$cpu" | wc -l | tr -d ' ')
[[ "$cpu_smap_sites" == 1 ]] || \
    fail "expected only the helper definition in cpu.rs, found $cpu_smap_sites sites"

if rg -Uq '\bas \*(const|mut) u(8|16|32|64)\)[[:space:]]*\.(read|write)(_volatile)?\(' "$cpu"; then
    fail 'cpu.rs directly dereferences a cast userspace scalar pointer'
fi

printf '%s\n' 'User-access architecture check passed'
