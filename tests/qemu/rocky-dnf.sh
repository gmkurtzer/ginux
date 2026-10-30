#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
output_file=$(mktemp "${TMPDIR:-/tmp}/ginux-rocky-dnf.XXXXXX")
volume_image=$(mktemp "${TMPDIR:-/tmp}/ginux-rocky-dnf-volume.XXXXXX")
qemu_pid_file=$(mktemp "${TMPDIR:-/tmp}/ginux-rocky-dnf-pid.XXXXXX")
qemu_pid=""

terminate_process() {
    local pid=$1
    [[ "$pid" =~ ^[0-9]+$ ]] || return 0
    kill -0 "$pid" 2>/dev/null || return 0
    kill "$pid" 2>/dev/null || true
    for _ in {1..20}; do
        kill -0 "$pid" 2>/dev/null || return 0
        sleep 0.1
    done
    kill -KILL "$pid" 2>/dev/null || true
}

stop_qemu() {
    local pid=""
    if [[ -s "$qemu_pid_file" ]]; then
        read -r pid <"$qemu_pid_file" || true
    fi
    terminate_process "$pid"
}

cleanup() {
    stop_qemu
    if [[ -n "$qemu_pid" ]]; then
        terminate_process "$qemu_pid"
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
        if [[ "$marker" == 'DNF_UPDATE_EXIT=0' ]] \
            && grep -Eq 'DNF_UPDATE_EXIT=[1-9][0-9]*' "$output_file"; then
            break
        fi
        if grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC|Linux init (execution|setup) failed|rocky: (failed|stopped)' "$output_file"; then
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
    wait_for 'ginux: entering configured Linux init' 30
    wait_for 'sh-5.2# ' 120
    printf '%s\n' "/usr/bin/readlink -f /bin/sh >/dev/null; printf 'REALPATH_RUNTIME_EXIT=%s\\n' \"\$?\""
    wait_for 'REALPATH_RUNTIME_EXIT=0' 300
    printf '%s\n' "value=\$(/usr/bin/printf x); printf 'SUBST_RUNTIME_VALUE=%s EXIT=%s\\n' \"\$value\" \"\$?\""
    wait_for 'SUBST_RUNTIME_VALUE=x EXIT=0' 300
    printf '%s\n' "/usr/bin/dd if=/dev/zero of=/tmp/ginux-overlay.bin bs=1024 count=64 status=none && test \"\$(/usr/bin/wc -c < /tmp/ginux-overlay.bin)\" -eq 65536; printf 'OVERLAY_RUNTIME_EXIT=%s\\n' \"\$?\""
    wait_for 'OVERLAY_RUNTIME_EXIT=0' 300
    printf '%s\n' "/usr/sbin/iconvconfig -o /tmp/ginux-gconv.cache; printf 'WRITEV_RUNTIME_EXIT=%s\\n' \"\$?\""
    wait_for 'WRITEV_RUNTIME_EXIT=0' 300
    printf '%s\n' "test \"\$(/usr/bin/cat /etc/rpm/platform)\" = x86_64-linux; printf 'RPM_PLATFORM_EXIT=%s\\n' \"\$?\""
    wait_for 'RPM_PLATFORM_EXIT=0' 300
    printf '%s\n' "/usr/bin/dnf -y update; printf 'DNF_UPDATE_EXIT=%s\\n' \"\$?\""
    wait_for 'DNF_UPDATE_EXIT=0' 2400
    printf '%s\n' 'exit 0'
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
    || grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC|Linux init (execution|setup) failed|Linux init exited with status 127' "$output_file"; then
    fail 'dnf update did not complete cleanly'
fi

echo 'Rocky dnf update test passed.'
