#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output_file="$(mktemp -t ginux-boot.XXXXXX)"
qemu_pid=""

cleanup() {
    if [[ -n "$qemu_pid" ]] && kill -0 "$qemu_pid" 2>/dev/null; then
        kill "$qemu_pid" 2>/dev/null || true
        wait "$qemu_pid" 2>/dev/null || true
    fi
    rm -f "$output_file"
}
trap cleanup EXIT

"$repo_root/tools/qemu/run.sh" >"$output_file" 2>&1 &
qemu_pid=$!

for _ in {1..30}; do
    if ! kill -0 "$qemu_pid" 2>/dev/null; then
        break
    fi
    sleep 1
done

if kill -0 "$qemu_pid" 2>/dev/null; then
    kill "$qemu_pid" 2>/dev/null || true
    wait "$qemu_pid" 2>/dev/null || true
    qemu_pid=""
    cat "$output_file"
    echo "Boot test failed: QEMU did not finish within 30 seconds." >&2
    exit 1
fi

if wait "$qemu_pid"; then
    qemu_status=0
else
    qemu_status=$?
fi
qemu_pid=""

cat "$output_file"

if ! grep -q '^GINUX_KERNEL_OK' "$output_file"; then
    echo "Boot test failed: the post-firmware kernel marker was not emitted." >&2
    exit 1
fi

if ! grep -Eq '^ginux: memory descriptors: [1-9][0-9]*' "$output_file"; then
    echo "Boot test failed: the kernel did not receive a valid UEFI memory map." >&2
    exit 1
fi

if ! grep -q '^ginux: kernel stack active' "$output_file"; then
    echo "Boot test failed: the kernel did not switch to its owned stack." >&2
    exit 1
fi

if ! grep -q '^ginux: versioned boot handoff accepted' "$output_file"; then
    echo "Boot test failed: the versioned boot handoff was not accepted." >&2
    exit 1
fi

if ! grep -q '^ginux: ACPI root pointer validated' "$output_file"; then
    echo "Boot test failed: the ACPI root pointer was not validated." >&2
    exit 1
fi

if ! grep -q '^ginux: signed boot manifest accepted' "$output_file"; then
    echo "Boot test failed: the signed boot manifest was not accepted." >&2
    exit 1
fi

if ! grep -q '^ginux: manifest signature rejection verified' "$output_file"; then
    echo "Boot test failed: an invalid manifest signature was not rejected." >&2
    exit 1
fi

if ! grep -q '^ginux: malformed boot manifests rejected' "$output_file"; then
    echo "Boot test failed: malformed canonical manifests were not rejected." >&2
    exit 1
fi

if ! grep -Eq '^ginux: manifest authority grants: [1-8]' "$output_file"; then
    echo "Boot test failed: the boot manifest did not provide bounded authority grants." >&2
    exit 1
fi

if ! grep -q '^ginux: control-plane bundle measurement accepted' "$output_file"; then
    echo "Boot test failed: the measured control-plane bundle was not accepted." >&2
    exit 1
fi

if ! grep -q '^ginux: kernel page tables active' "$output_file"; then
    echo "Boot test failed: the kernel did not activate its own page tables." >&2
    exit 1
fi

if ! grep -q '^ginux: null and stack guard pages active' "$output_file"; then
    echo "Boot test failed: hardened null/stack mappings are not active." >&2
    exit 1
fi

if ! grep -q '^ginux: ACPI MADT validated' "$output_file"; then
    echo "Boot test failed: ACPI MADT discovery did not complete." >&2
    exit 1
fi

if ! grep -Eq '^ginux: enabled processors: [1-9][0-9]*' "$output_file"; then
    echo "Boot test failed: ACPI reported no enabled processors." >&2
    exit 1
fi

if ! grep -Eq '^ginux: I/O APICs: [1-9][0-9]*' "$output_file"; then
    echo "Boot test failed: ACPI reported no I/O APICs." >&2
    exit 1
fi

if ! grep -q '^ginux: GDT/TSS/IDT active' "$output_file"; then
    echo "Boot test failed: kernel CPU protection tables are not active." >&2
    exit 1
fi

if ! grep -q '^ginux: architectural exception gates active' "$output_file"; then
    echo "Boot test failed: architectural exceptions are not fully covered." >&2
    exit 1
fi

if ! grep -q '^ginux: breakpoint exception handled' "$output_file"; then
    echo "Boot test failed: the recoverable exception test did not return." >&2
    exit 1
fi

if ! grep -q '^ginux: local APIC discovered' "$output_file"; then
    echo "Boot test failed: the local APIC was not discovered." >&2
    exit 1
fi

if ! grep -Eq '^ginux: local APIC mode: x2?APIC' "$output_file"; then
    echo "Boot test failed: the local APIC mode is invalid." >&2
    exit 1
fi

if ! grep -Eq '^ginux: local APIC physical base: [1-9][0-9]*' "$output_file"; then
    echo "Boot test failed: the local APIC base was not reported." >&2
    exit 1
fi

if ! grep -q '^ginux: timer capabilities discovered' "$output_file"; then
    echo "Boot test failed: timer capabilities were not discovered." >&2
    exit 1
fi

if ! grep -Eq '^ginux: invariant TSC: (advertised|not-advertised)' "$output_file"; then
    echo "Boot test failed: invariant-TSC capability is invalid." >&2
    exit 1
fi

if ! grep -Eq '^ginux: TSC deadline timer: (supported|unsupported)' "$output_file"; then
    echo "Boot test failed: TSC-deadline capability is invalid." >&2
    exit 1
fi

if ! grep -Eq '^ginux: TSC frequency source: (architectural-ratio|nominal-processor-frequency|unavailable)' "$output_file"; then
    echo "Boot test failed: TSC frequency source is invalid." >&2
    exit 1
fi

if ! grep -q '^ginux: local APIC timer calibrated' "$output_file"; then
    echo "Boot test failed: the local APIC timer was not calibrated." >&2
    exit 1
fi

if ! grep -Eq '^ginux: APIC timer counter Hz: [1-9][0-9]*' "$output_file"; then
    echo "Boot test failed: the local APIC timer frequency is invalid." >&2
    exit 1
fi

if ! grep -q '^ginux: local APIC timer interrupt handled' "$output_file"; then
    echo "Boot test failed: the local APIC timer interrupt was not handled." >&2
    exit 1
fi

if ! grep -q '^ginux: capability derivation and revocation verified' "$output_file"; then
    echo "Boot test failed: capability authority invariants failed." >&2
    exit 1
fi

if ! grep -q '^ginux: persistent control-plane domain sealed' "$output_file"; then
    echo "Boot test failed: persistent control-plane domain state was not sealed." >&2
    exit 1
fi

if ! grep -Eq '^ginux: control-plane domain ID: [1-9][0-9]*' "$output_file"; then
    echo "Boot test failed: control-plane domain identity is invalid." >&2
    exit 1
fi

if ! grep -Eq '^ginux: usable memory MiB: [1-9][0-9]*' "$output_file"; then
    echo "Boot test failed: the kernel did not inventory usable physical memory." >&2
    exit 1
fi

if ! grep -q '^ginux: physical frame allocator active' "$output_file"; then
    echo "Boot test failed: physical frame allocation was not verified." >&2
    exit 1
fi

if ! grep -q '^ginux: isolated address space active' "$output_file"; then
    echo "Boot test failed: isolated address-space activation failed." >&2
    exit 1
fi

if ! grep -q '^ginux: control-plane W\^X mappings active' "$output_file"; then
    echo "Boot test failed: control-plane W^X mappings are not active." >&2
    exit 1
fi

if ! grep -q '^ginux: bounded executable container accepted' "$output_file"; then
    echo "Boot test failed: the executable container was not accepted." >&2
    exit 1
fi

if ! grep -q '^ginux: malformed executable containers rejected' "$output_file"; then
    echo "Boot test failed: malformed executable containers were accepted." >&2
    exit 1
fi

if [[ "$(grep -c '^USER_OK' "$output_file")" -ne 2 ]]; then
    echo "Boot test failed: userspace console writes were not emitted exactly twice." >&2
    exit 1
fi

if ! grep -q '^ginux: opaque capability handles enforced at call gate' "$output_file"; then
    echo "Boot test failed: opaque capability-handle enforcement failed." >&2
    exit 1
fi

if ! grep -q '^ginux: control call ABI v1 negotiated' "$output_file"; then
    echo "Boot test failed: control call ABI negotiation failed." >&2
    exit 1
fi

if ! grep -q '^ginux: userspace console capability enforced' "$output_file"; then
    echo "Boot test failed: userspace console authority failed." >&2
    exit 1
fi

if ! grep -q '^ginux: ring-3 fault survivor resumed and exited' "$output_file"; then
    echo "Boot test failed: the surviving ring-3 context did not complete." >&2
    exit 1
fi

if ! grep -q '^ginux: user invalid-opcode fault contained' "$output_file"; then
    echo "Boot test failed: a ring-3 fault was not contained." >&2
    exit 1
fi

if ! grep -q '^ginux: isolated scheduler address spaces switched' "$output_file"; then
    echo "Boot test failed: scheduler address-space switching failed." >&2
    exit 1
fi

if ! grep -q '^ginux: capability-gated bounded IPC verified' "$output_file"; then
    echo "Boot test failed: bounded IPC authority or FIFO behavior failed." >&2
    exit 1
fi

if ! grep -q '^ginux: per-thread syscall budgets enforced' "$output_file"; then
    echo "Boot test failed: per-thread syscall accounting failed." >&2
    exit 1
fi

if ! grep -q '^ginux: scheduler context lifecycle verified' "$output_file"; then
    echo "Boot test failed: scheduler context lifecycle invariants failed." >&2
    exit 1
fi

if ! grep -q '^ginux: bootstrap SHA-256 known answers verified' "$output_file"; then
    echo "Boot test failed: bootstrap SHA-256 known-answer tests failed." >&2
    exit 1
fi

if grep -qE '^GINUX_(BOOT|KERNEL)_PANIC' "$output_file"; then
    echo "Boot test failed: a panic marker was emitted." >&2
    exit 1
fi

if [[ "$qemu_status" -ne 0 ]]; then
    echo "Boot test failed: QEMU exited with $qemu_status; expected 0." >&2
    exit 1
fi

echo "Boot test passed."
