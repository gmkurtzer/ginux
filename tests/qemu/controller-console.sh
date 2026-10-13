#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output_file="$(mktemp -t ginux-controller.XXXXXX)"
volume_image="$(mktemp -t ginux-controller-volume.XXXXXX)"
qemu_pid=""

cleanup() {
    if [[ -n "$qemu_pid" ]] && kill -0 "$qemu_pid" 2>/dev/null; then
        kill "$qemu_pid" 2>/dev/null || true
        wait "$qemu_pid" 2>/dev/null || true
    fi
    rm -f "$output_file" "$volume_image"
}
trap cleanup EXIT

dd if=/dev/zero of="$volume_image" bs=1048576 count=1 2>/dev/null
{
    for _ in {1..60}; do
        if grep -Fq 'ginux> ' "$output_file"; then
            break
        fi
        sleep 1
    done
    printf 'help\nstatus\ncomponents\nls\nls /etc\nls /proc\nls /proc/1\nps\nunknown\nshutdown\n'
} | GINUX_VOLUME_IMAGE="$volume_image" "$repo_root/tools/qemu/run.sh" >"$output_file" 2>&1 &
qemu_pid=$!

for _ in {1..75}; do
    if ! kill -0 "$qemu_pid" 2>/dev/null; then
        break
    fi
    sleep 1
done

if kill -0 "$qemu_pid" 2>/dev/null; then
    cat "$output_file"
    echo "Controller console test failed: QEMU did not shut down within 75 seconds." >&2
    exit 1
fi

if wait "$qemu_pid"; then
    status=0
else
    status=$?
    cat "$output_file"
    echo "Controller console test failed: QEMU exited with $status." >&2
    exit 1
fi
qemu_pid=""

for expected in \
    'ginux: embedded controller runtime measurement accepted' \
    'ginux: launching embedded controller in ring 3' \
    'Ginux controller v0 (ring 3)' \
    'help        Show this command list' \
    'ls [PATH]   List the embedded measured root' \
    'ps          Show live native domains' \
    'controller: running (ring 3)' \
    'containers: 0 (C1)' \
    'integrity: sealed' \
    'GINUX.EFI component catalog v0:' \
    '- kernel: active, ring 0' \
    '- controller: active, ring 3' \
    '- boot modules: embedded, sealed' \
    'bin/' \
    'etc/' \
    'sbin/' \
    'hostname' \
    'os-release' \
    'motd' \
    '1/' \
    'status' \
    'PID PPID STATE NAME' \
    '1 0 R controller' \
    "Unknown command. Type 'help'." \
    'Shutting down Ginux.' \
    'ginux: controller requested clean shutdown' \
    'GINUX_KERNEL_OK'; do
    if ! grep -Fq -- "$expected" "$output_file"; then
        cat "$output_file"
        echo "Controller console test failed: missing '$expected'." >&2
        exit 1
    fi
done

if grep -qE '^GINUX_(BOOT|KERNEL)_PANIC' "$output_file"; then
    cat "$output_file"
    echo "Controller console test failed: a panic marker was emitted." >&2
    exit 1
fi

echo "Interactive controller console test passed."
