#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
output_file="${GINUX_MULTI_DOMAIN_LOG:-}"
output_file_is_temporary=0
if [[ -z "$output_file" ]]; then
    output_file=$(mktemp "${TMPDIR:-/tmp}/ginux-multi-domain.XXXXXX")
    output_file_is_temporary=1
fi
volume_image=$(mktemp "${TMPDIR:-/tmp}/ginux-multi-domain-volume.XXXXXX")
qemu_pid_file=$(mktemp "${TMPDIR:-/tmp}/ginux-multi-domain-pid.XXXXXX")
qemu_pid=""
base_volume="${GINUX_MULTI_DOMAIN_BASE_VOLUME:-$repo_root/build/qemu/rocky-systemd-volume.raw}"

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
    [[ "$output_file_is_temporary" == 0 ]] || rm -f "$output_file"
    rm -f "$volume_image" "$qemu_pid_file"
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
    echo "Multi-domain test failed while waiting for '$marker'." >&2
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
    echo "Multi-domain test failed while waiting for $expected occurrences of '$marker'." >&2
    stop_qemu
    return 1
}

if [[ ! -f "$base_volume" ]]; then
    echo "Multi-domain test requires a cached Rocky volume: $base_volume" >&2
    echo "Run 'make test-rocky-systemd' first or set GINUX_MULTI_DOMAIN_BASE_VOLUME." >&2
    exit 1
fi
if ! cp -c "$base_volume" "$volume_image" 2>/dev/null; then
    if ! cp --sparse=always "$base_volume" "$volume_image" 2>/dev/null; then
        cp "$base_volume" "$volume_image"
    fi
fi
: >"$output_file"
{
    wait_for 'ginux> '
    printf '%s\n' \
        'domain new rocky' \
        'domain set rocky image=rockylinux/rockylinux:10-ubi-init init=/sbin/init memory-mib=768' \
        'domain start rocky --attach'
    wait_for 'ginux: OCI image ready' 300
    wait_for 'ginux login:' 180
    printf '\035'
    wait_for "[detached from domain; use 'domain attach NAME' to return]" 10
    printf '%s\n' \
        'domain new rocky2' \
        'domain set rocky2 image=rockylinux/rockylinux:10-ubi-init init=/sbin/init memory-mib=768' \
        'domain start rocky2 --attach'
    wait_for_count 'ginux login:' 2 180
    printf '\035'
    wait_for_count "[detached from domain; use 'domain attach NAME' to return]" 2 10
    wait_for_count 'ginux: OCI image ready' 2 300
    wait_for_count 'ginux: entering configured Linux init' 2 30
    printf '%s\n' 'domain list'
    wait_for 'rocky           running     linux'
    wait_for 'rocky2          running     linux'
    printf '%s\n' 'domain exec rocky /bin/true'
    wait_for_count 'rocky: command completed' 1
    printf '%s\n' 'domain exec rocky2 /bin/true'
    wait_for_count 'rocky2: command completed' 1
    printf '%s\n' 'domain stop rocky'
    wait_for 'rocky: stopped'
    printf '%s\n' 'domain exec rocky2 /bin/true'
    wait_for_count 'rocky2: command completed' 2
    printf '%s\n' 'domain stop rocky2'
    wait_for 'rocky2: stopped'
    printf '%s\n' \
        'domain list' \
        'shutdown'
} | GINUX_MEMORY="${GINUX_MEMORY:-2G}" GINUX_VOLUME_IMAGE="$volume_image" GINUX_QEMU_PID_FILE="$qemu_pid_file" \
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
    echo "Multi-domain test failed: QEMU did not shut down." >&2
    exit 1
fi
wait "$qemu_pid" || true
qemu_pid=""

for expected in 'GINUX_KERNEL_OK'; do
    if ! grep -Fq "$expected" "$output_file"; then
        tail -n 160 "$output_file"
        echo "Multi-domain test failed: missing '$expected'." >&2
        exit 1
    fi
done

if grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC|live registry refresh snapshot failed' "$output_file"; then
    tail -n 160 "$output_file"
    echo "Multi-domain test failed: kernel or process-registry failure detected." >&2
    exit 1
fi

echo "Concurrent Rocky domain lifecycle test passed."
