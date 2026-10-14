#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
output_file=$(mktemp "${TMPDIR:-/tmp}/ginux-alpine-qemu.XXXXXX")
volume_image=$(mktemp "${TMPDIR:-/tmp}/ginux-alpine-volume.XXXXXX")
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
    for _ in {1..180}; do
        if grep -Fq 'ginux> ' "$output_file"; then
            printf 'shutdown\n'
            exit 0
        fi
        sleep 1
    done
} | GINUX_VOLUME_IMAGE="$volume_image" "$repo_root/tools/qemu/run.sh" >"$output_file" 2>&1 &
qemu_pid=$!

for _ in {1..210}; do
    if grep -Fq 'GINUX_KERNEL_OK' "$output_file" || grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC' "$output_file"; then
        break
    fi
    if ! kill -0 "$qemu_pid" 2>/dev/null; then
        break
    fi
    sleep 1
done

for _ in {1..10}; do
    ! kill -0 "$qemu_pid" 2>/dev/null && break
    sleep 1
done

if kill -0 "$qemu_pid" 2>/dev/null; then
    cat "$output_file"
    echo "Alpine image test failed: QEMU did not shut down." >&2
    exit 1
fi
wait "$qemu_pid"
qemu_pid=""

for expected in \
    'ginux: pinned Alpine OCI rootfs accepted' \
    'ginux: OCI init and declared ELF interpreter discovered' \
    'ginux: OCI relocatable PIE load plan accepted' \
    'Ginux controller v0 (ring 3)' \
    'ginux: controller requested clean shutdown' \
    'GINUX_KERNEL_OK'; do
    if ! grep -Fq "$expected" "$output_file"; then
        cat "$output_file"
        echo "Alpine image test failed: missing '$expected'." >&2
        exit 1
    fi
done

if grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC' "$output_file"; then
    cat "$output_file"
    echo "Alpine image test failed: a panic marker was emitted." >&2
    exit 1
fi

echo "Pinned Alpine OCI boot ingestion test passed."
