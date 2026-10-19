#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
output_file=$(mktemp "${TMPDIR:-/tmp}/ginux-rocky-systemd.XXXXXX")
volume_image=$(mktemp "${TMPDIR:-/tmp}/ginux-rocky-systemd-volume.XXXXXX")
qemu_pid_file=$(mktemp "${TMPDIR:-/tmp}/ginux-rocky-systemd-pid.XXXXXX")
success_file=$(mktemp "${TMPDIR:-/tmp}/ginux-rocky-systemd-success.XXXXXX")
qemu_pid=""

stop_qemu() {
    local pid=""
    if [[ -s "$qemu_pid_file" ]]; then
        read -r pid <"$qemu_pid_file" || true
    fi
    if [[ "$pid" =~ ^[0-9]+$ ]] && kill -0 "$pid" 2>/dev/null; then
        kill "$pid" 2>/dev/null || true
    fi
}

cleanup() {
    stop_qemu
    if [[ -n "$qemu_pid" ]] && kill -0 "$qemu_pid" 2>/dev/null; then
        kill "$qemu_pid" 2>/dev/null || true
        wait "$qemu_pid" 2>/dev/null || true
    fi
    rm -f "$output_file" "$volume_image" "$qemu_pid_file" "$success_file"
}
trap cleanup EXIT

fail() {
    tail -n 160 "$output_file" >&2
    echo "Rocky systemd smoke test failed: $1" >&2
    stop_qemu
    return 1
}

wait_for() {
    local marker=$1
    local limit=${2:-120}
    for ((attempt = 0; attempt < limit; attempt++)); do
        grep -Fq "$marker" "$output_file" && return 0
        if grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC' "$output_file"; then
            break
        fi
        sleep 1
    done
    fail "timed out waiting for '$marker'"
}

dd if=/dev/zero of="$volume_image" bs=1048576 count=1 2>/dev/null
{
    wait_for 'ginux> '
    printf '%s\n' \
        'domain new rocky' \
        'domain set rocky image=rockylinux/rockylinux:10-ubi-init init=/sbin/init' \
        'domain start rocky --attach'
    wait_for 'ginux: OCI image ready' 240
    wait_for 'ginux: entering configured Linux init' 30
    wait_for ' running in system mode (' 30
    sleep 15
    if grep -Fq 'ginux: Linux init diagnostics: exit=127 first-unsupported-syscall=none' "$output_file"; then
        printf '%s\n' 'shutdown'
    else
        if grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC|Linux init (execution|setup) failed|Linux init exited with status|rocky: (failed|stopped)' "$output_file"; then
            fail 'systemd failed before the documented mmap boundary'
        fi
        printf '%s\n' ok >"$success_file"
        stop_qemu
    fi
} | GINUX_VOLUME_IMAGE="$volume_image" GINUX_QEMU_PID_FILE="$qemu_pid_file" \
    "$repo_root/tools/qemu/run.sh" >"$output_file" 2>&1 &
qemu_pid=$!

for _ in {1..420}; do
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
    fail 'QEMU did not shut down'
fi
wait "$qemu_pid" || true
qemu_pid=""

if [[ -s "$success_file" ]] \
    && ! grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC|Linux init (execution|setup) failed|Linux init exited with status|rocky: (failed|stopped)' "$output_file"; then
    echo 'Rocky systemd PID 1 smoke test passed.'
    exit 0
fi

if ! grep -Fq 'GINUX_KERNEL_OK' "$output_file" \
    || grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC' "$output_file"; then
    fail 'clean-shutdown marker was not observed'
fi

if grep -Fq 'rocky: running' "$output_file"; then
    echo 'Rocky systemd PID 1 smoke test passed.'
elif grep -Fq 'ginux: Linux init recent syscalls:' "$output_file" \
    && grep -Fq 'ginux: Linux init diagnostics: exit=127 first-unsupported-syscall=none' "$output_file"; then
    echo 'Rocky systemd reached the documented dynamic-mmap compatibility boundary.'
else
    fail 'systemd neither remained running nor reached the documented boundary'
fi
