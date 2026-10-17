#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
output_file=$(mktemp "${TMPDIR:-/tmp}/ginux-rocky-dnf.XXXXXX")
volume_image=$(mktemp "${TMPDIR:-/tmp}/ginux-rocky-dnf-volume.XXXXXX")
qemu_pid_file=$(mktemp "${TMPDIR:-/tmp}/ginux-rocky-dnf-pid.XXXXXX")
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
    rm -f "$output_file" "$volume_image" "$qemu_pid_file"
}
trap cleanup EXIT

fail() {
    grep -F 'Linux syscall returned EFAULT' "$output_file" >&2 || true
    grep -E 'Errno|Error:|Failed|failed|unsupported|not implemented|transaction|fault|DNF_UPDATE' "$output_file" \
        | tail -n 80 >&2 || true
    tr '\r' '\n' <"$output_file" | tail -n 160 >&2
    echo "Rocky dnf test failed: $1" >&2
    stop_qemu
    return 1
}

wait_for() {
    local marker=$1
    local limit=${2:-120}
    local observed_pid=""
    for ((attempt = 0; attempt < limit; attempt++)); do
        grep -Fq "$marker" "$output_file" && return 0
        if grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC' "$output_file"; then
            break
        fi
        if [[ -s "$qemu_pid_file" ]]; then
            read -r observed_pid <"$qemu_pid_file" || true
            if [[ "$observed_pid" =~ ^[0-9]+$ ]] && ! kill -0 "$observed_pid" 2>/dev/null; then
                break
            fi
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
        'domain set rocky image=rockylinux/rockylinux:10 init=/bin/sh memory-mib=2048' \
        'domain start rocky --attach'
    wait_for 'ginux: OCI image ready' 300
    wait_for 'ginux: entering configured OCI init' 30
    printf '%s\n' \
        "/usr/bin/readlink -f /bin/sh >/dev/null; printf 'REALPATH_RUNTIME_EXIT=%s\\n' \"\$?\"" \
        "/usr/bin/dd if=/dev/zero of=/tmp/ginux-overlay.bin bs=1024 count=64 status=none && test \"\$(/usr/bin/wc -c < /tmp/ginux-overlay.bin)\" -eq 65536; printf 'OVERLAY_RUNTIME_EXIT=%s\\n' \"\$?\"" \
        "/usr/sbin/iconvconfig -o /tmp/ginux-gconv.cache; printf 'WRITEV_RUNTIME_EXIT=%s\\n' \"\$?\""
    wait_for 'REALPATH_RUNTIME_EXIT=0' 300
    wait_for 'OVERLAY_RUNTIME_EXIT=0' 300
    wait_for 'WRITEV_RUNTIME_EXIT=0' 300
    printf '%s\n' \
        '/usr/bin/dnf -y update' \
        "printf 'DNF_UPDATE_EXIT=%s\\n' \"\$?\"" \
        'exit 0'
    wait_for 'DNF_UPDATE_EXIT=' 2400
    wait_for 'rocky: stopped' 120
    printf '%s\n' 'shutdown'
} | GINUX_MEMORY="${GINUX_MEMORY:-2G}" GINUX_VOLUME_IMAGE="$volume_image" GINUX_QEMU_PID_FILE="$qemu_pid_file" \
    "$repo_root/tools/qemu/run.sh" >"$output_file" 2>&1 &
qemu_pid=$!

for _ in {1..3000}; do
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

if ! grep -Fq 'GINUX_KERNEL_OK' "$output_file" \
    || ! grep -Fq 'DNF_UPDATE_EXIT=0' "$output_file" \
    || ! grep -Fq 'WRITEV_RUNTIME_EXIT=0' "$output_file" \
    || grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC|OCI init (execution|setup) failed|OCI init exited with status 127' "$output_file"; then
    fail 'dnf update did not complete cleanly'
fi

echo 'Rocky dnf update test passed.'
