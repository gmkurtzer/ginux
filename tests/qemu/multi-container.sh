#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
output_file=$(mktemp "${TMPDIR:-/tmp}/ginux-multi-container.XXXXXX")
volume_image=$(mktemp "${TMPDIR:-/tmp}/ginux-multi-container-volume.XXXXXX")
qemu_pid_file=$(mktemp "${TMPDIR:-/tmp}/ginux-multi-container-pid.XXXXXX")
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
    tail -n 160 "$output_file" >&2
    echo "Multi-container test failed while waiting for '$marker'." >&2
    stop_qemu
    return 1
}

wait_for_count() {
    local marker=$1
    local expected=$2
    local limit=${3:-120}
    for ((attempt = 0; attempt < limit; attempt++)); do
        local observed
        observed=$(grep -Fc "$marker" "$output_file" || true)
        [[ "$observed" -ge "$expected" ]] && return 0
        if grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC' "$output_file"; then
            break
        fi
        sleep 1
    done
    tail -n 160 "$output_file" >&2
    echo "Multi-container test failed while waiting for $expected occurrences of '$marker'." >&2
    stop_qemu
    return 1
}

dd if=/dev/zero of="$volume_image" bs=1048576 count=1 2>/dev/null
{
    wait_for 'ginux> '
    printf '%s\n' \
        'container new alpine' \
        'container set alpine image=alpine:latest init=/bin/sh' \
        'container start alpine'
    wait_for 'alpine: running (detached)'
    printf '%s\n' \
        'container new alpine2' \
        'container set alpine2 image=alpine:latest init=/bin/sh' \
        'container start alpine2'
    wait_for 'alpine2: running (detached)'
    printf '%s\n' \
        'container new rocky' \
        'container set rocky image=rockylinux/rockylinux:10 init=/usr/bin/true' \
        'container start rocky'
    wait_for_count 'ginux: OCI image ready' 3 240
    wait_for_count 'ginux: entering configured OCI init' 3 30
    wait_for 'rocky: stopped'
    printf '%s\n' 'container list' 'ps'
    wait_for 'alpine          running     /bin/sh               1    128      false  docker.io/library/alpine:latest'
    wait_for 'alpine2         running     /bin/sh               1    128      false  docker.io/library/alpine:latest'
    wait_for 'rocky           stopped     /usr/bin/true         1    128      false  docker.io/rockylinux/rockylinux:10'
    wait_for '100    0      R      alpine:init'
    wait_for '101    0      R      alpine2:init'
    printf '%s\n' 'container attach alpine'
    wait_for $'\033[2J\033[H'
    printf '%s\n' \
        "printf 'ALPINE_DOMAIN_OK\\n'" \
        "test -f /proc/1/status && printf 'ALPINE_PROC_OK\\n'"
    wait_for 'ALPINE_DOMAIN_OK'
    wait_for 'ALPINE_PROC_OK'
    printf '\035'
    wait_for "[detached from container; use 'container attach NAME' to return]"
    printf '%s\n' 'container attach alpine2'
    printf '%s\n' "printf 'ALPINE2_DOMAIN_OK\\n'"
    wait_for 'ALPINE2_DOMAIN_OK'
    printf '\035'
    wait_for_count "[detached from container; use 'container attach NAME' to return]" 2 30
    printf '%s\n' 'container attach alpine'
    printf '%s\n' \
        "printf 'ALPINE_RESUME_OK\\n'" \
        'exit 0'
    wait_for 'ALPINE_RESUME_OK'
    wait_for 'alpine: stopped'
    printf '%s\n' 'container stop alpine2'
    wait_for 'alpine2: stopped'
    printf '%s\n' \
        'container list' \
        'shutdown'
} | GINUX_VOLUME_IMAGE="$volume_image" GINUX_QEMU_PID_FILE="$qemu_pid_file" \
    "$repo_root/tools/qemu/run.sh" >"$output_file" 2>&1 &
qemu_pid=$!

for _ in {1..600}; do
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
    tail -n 160 "$output_file"
    echo "Multi-container test failed: QEMU did not shut down." >&2
    exit 1
fi
wait "$qemu_pid" || true
qemu_pid=""

for expected in \
    'ALPINE_DOMAIN_OK' \
    'ALPINE_PROC_OK' \
    'ALPINE2_DOMAIN_OK' \
    'ALPINE_RESUME_OK' \
    'GINUX_KERNEL_OK'; do
    if ! grep -Fq "$expected" "$output_file"; then
        tail -n 160 "$output_file"
        echo "Multi-container test failed: missing '$expected'." >&2
        exit 1
    fi
done

if grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC|live registry refresh snapshot failed' "$output_file"; then
    tail -n 160 "$output_file"
    echo "Multi-container test failed: kernel or process-registry failure detected." >&2
    exit 1
fi

echo "Concurrent Alpine and Rocky domain lifecycle test passed."
