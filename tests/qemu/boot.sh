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

if ! grep -q '^ginux: kernel page tables active' "$output_file"; then
    echo "Boot test failed: the kernel did not activate its own page tables." >&2
    exit 1
fi

if ! grep -q '^ginux: null and stack guard pages active' "$output_file"; then
    echo "Boot test failed: hardened null/stack mappings are not active." >&2
    exit 1
fi

if ! grep -q '^ginux: GDT/TSS/IDT active' "$output_file"; then
    echo "Boot test failed: kernel CPU protection tables are not active." >&2
    exit 1
fi

if ! grep -q '^ginux: breakpoint exception handled' "$output_file"; then
    echo "Boot test failed: the recoverable exception test did not return." >&2
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

if grep -qE '^GINUX_(BOOT|KERNEL)_PANIC' "$output_file"; then
    echo "Boot test failed: a panic marker was emitted." >&2
    exit 1
fi

if [[ "$qemu_status" -ne 0 ]]; then
    echo "Boot test failed: QEMU exited with $qemu_status; expected 0." >&2
    exit 1
fi

echo "Boot test passed."
