#!/usr/bin/env bash

set -euo pipefail

root=$(git rev-parse --show-toplevel)

"$root/tools/architecture/inventory-writable-globals.sh" | awk -F '\t' '
    BEGIN {
        OFS = "\t"
        print "kind", "file", "symbol", "ownership-group", "disposition", "migration-gate", "contract"
    }
    NR == 1 { next }
    {
        kind = $1
        file = $2
        symbol = $4
        group = "shared-runtime"
        disposition = "migrate"
        gate = "5"
        contract = "kernel-runtime-manager-owned"

        if (file ~ /^boot\//) {
            group = "bootstrap-init"
            disposition = "allowlist"
            gate = "none"
            contract = "firmware-single-writer"
        } else if (file ~ /^services\//) {
            group = "service-runtime"
            gate = "6"
            contract = "isolated-service-generation-owned"
        } else if (file == "kernel/src/arch/x86_64/cpu.rs") {
            if (symbol ~ /^(CPU_ENTRY_STATES|GDT|TSS|IDT|PRIVILEGE_STACK|DOUBLE_FAULT_STACK|NMI_STACK|MACHINE_CHECK_STACK|EXTENDED_USER_STATE_ENABLED)$/) {
                group = "per-cpu-arch"
                gate = "6"
                contract = "cpu-local-entry-state"
            } else if (symbol ~ /^(BREAKPOINT_HANDLED|SYSCALL_STATE|CONTROL_DISCOVERY_OVERRIDE_|CONTROL_DESCRIPTOR_OVERRIDE_|PRIVILEGE_ENTRY_|LINUX_ABI_TEST_RESULTS|SERVICE_STACK_SCRUBBED|SERVICE_FRAMES_TRANSFERRED)/) {
                group = "verification-only"
                disposition = "allowlist"
                gate = "none"
                contract = "verification-build-only"
            } else if (symbol ~ /^(ELF_PROBE_|LINUX_THREAD_|LINUX_PROCESS_|LINUX_CURRENT_(PROCESS|THREAD)|LINUX_NEXT_(TID|OPEN_FILE)|LINUX_FORK_|LINUX_(FIRST_)?CHILD_|LINUX_PARENT_|LINUX_CLONE_|LINUX_FUTEX_|LINUX_SLEEP_|LINUX_TIMER_WAIT_|LINUX_POLL_|LINUX_EPOLL_|LINUX_EVENTFD_|LINUX_PIPE_|LINUX_TIMERFD_|LINUX_PPOLL_|LINUX_MASKED_|LINUX_SIGNAL_WAIT_|LINUX_CROSS_PROCESS_|LINUX_FSIZE_|LINUX_INOTIFY_|LINUX_FANOTIFY_|LINUX_IO_URING_|LINUX_LAST_PREEMPTED_|LINUX_LIVE_RETURN_|LINUX_TTY_|LINUX_SYSTEM_SUPERVISOR_|LINUX_(TIMED|DESCRIPTOR)_WAITS_|LINUX_YIELD_|PROCESS_CHILD_|LIVE_FORK_)/) {
                group = "process-runtime"
                gate = "5"
                contract = "process-or-thread-generation-owned"
            } else if (symbol ~ /^(CONTAINER_|DOMAIN_|LINUX_|NETWORK_)/) {
                group = "domain-runtime"
                gate = "5"
                contract = "domain-generation-owned"
            }
        } else if (file == "kernel/src/address_space.rs") {
            if (symbol ~ /^LINUX_PROCESS_/) {
                group = "process-runtime"
                gate = "5"
                contract = "process-or-thread-generation-owned"
            } else if (symbol ~ /^LINUX_RUNTIME_/) {
                group = "domain-runtime"
                gate = "5"
                contract = "domain-generation-owned"
            } else {
                group = "platform-device"
                gate = "6"
                contract = "device-or-host-service-owned"
            }
        } else if (file == "kernel/src/linux_process.rs" || file == "kernel/src/linux_signal.rs") {
            group = "process-runtime"
            gate = "5"
            contract = "process-or-thread-generation-owned"
        } else if (file ~ /^kernel\/src\/linux_/) {
            group = "domain-runtime"
            gate = "5"
            contract = "domain-generation-owned"
        } else if (file == "kernel/src/domain.rs" || file == "kernel/src/scheduler.rs" || file == "kernel/src/ipc.rs") {
            group = "shared-runtime"
            gate = "5"
            contract = "kernel-runtime-manager-owned"
        } else if (file ~ /^kernel\/src\/arch\/x86_64\/(paging|pci|timer)\.rs$/ ||
                   file ~ /^kernel\/src\/(memory|network|packet|volume|image_store|initramfs|published_rootfs)\.rs$/ ||
                   file == "kernel/src/lib.rs") {
            group = "platform-device"
            gate = "6"
            contract = "device-or-host-service-owned"
        }

        print kind, file, symbol, group, disposition, gate, contract
    }
' 
