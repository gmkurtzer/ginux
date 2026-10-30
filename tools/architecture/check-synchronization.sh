#!/usr/bin/env bash
set -euo pipefail

root=$(git rev-parse --show-toplevel)

fail() {
    printf 'synchronization check: %s\n' "$*" >&2
    exit 1
}

require_text() {
    local file=$1
    local text=$2
    grep -Fq -- "$text" "$file" || fail "missing '$text' in ${file#"$root/"}"
}

require_text "$root/kernel/src/arch/x86_64/cpu/linux_syscall.rs" \
    'let _kernel_entry = crate::sync::kernel_entry_lock();'
require_text "$root/kernel/src/arch/x86_64/cpu.rs" \
    'extern "sysv64" fn syscall_handler'
require_text "$root/kernel/src/arch/x86_64/cpu.rs" \
    'extern "sysv64" fn user_fault_handler'
require_text "$root/kernel/src/arch/x86_64/timer.rs" \
    'let _kernel_entry = crate::sync::kernel_entry_lock();'
require_text "$root/kernel/src/arch/x86_64/pci.rs" \
    'let _kernel_entry = crate::sync::kernel_entry_lock();'
require_text "$root/kernel/src/memory.rs" \
    'allocator: IrqSpinLock<MaybeUninit<FrameAllocator>>'
require_text "$root/kernel/src/memory.rs" \
    'if !allocator.can_release_contiguous_extent(owner, first, count)'
require_text "$root/kernel/src/sync.rs" \
    'static KERNEL_ENTRY_LOCK: KernelEntryLock = KernelEntryLock::new();'

if grep -Fq -- 'single-CPU lifecycle serialization' "$root/kernel/src/linux_process.rs"; then
    fail 'process runtime still claims single-CPU serialization'
fi

entry_count=$(rg -l 'let _kernel_entry = crate::sync::kernel_entry_lock\(\);' \
    "$root/kernel/src/arch/x86_64/cpu.rs" \
    "$root/kernel/src/arch/x86_64/cpu/linux_syscall.rs" \
    "$root/kernel/src/arch/x86_64/timer.rs" \
    "$root/kernel/src/arch/x86_64/pci.rs" | wc -l | tr -d ' ')
[[ "$entry_count" == 4 ]] || fail 'not all four entry-owner files retain the shared boundary'

printf 'Synchronization architecture check passed\n'
