#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
output_file="${GINUX_ROCKY_LOG:-$repo_root/build/qemu/rocky-systemd.log}"
volume_image="${GINUX_VOLUME_IMAGE:-$repo_root/build/qemu/rocky-systemd-volume.raw}"
qemu_pid_file=$(mktemp "${TMPDIR:-/tmp}/ginux-rocky-systemd-pid.XXXXXX")
success_file=$(mktemp "${TMPDIR:-/tmp}/ginux-rocky-systemd-success.XXXXXX")
qemu_pid=""
rocky_image="${GINUX_ROCKY_IMAGE:-rockylinux/rockylinux:10-ubi-init}"

stop_qemu() {
    local pid=""
    if [[ -s "$qemu_pid_file" ]]; then
        read -r pid <"$qemu_pid_file" || true
    fi
    if [[ "$pid" =~ ^[0-9]+$ ]] && kill -0 "$pid" 2>/dev/null; then
        kill "$pid" 2>/dev/null || true
        for _ in {1..20}; do
            ! kill -0 "$pid" 2>/dev/null && return 0
            sleep 0.1
        done
        kill -KILL "$pid" 2>/dev/null || true
    fi
}

cleanup() {
    stop_qemu
    if [[ -n "$qemu_pid" ]] && kill -0 "$qemu_pid" 2>/dev/null; then
        kill "$qemu_pid" 2>/dev/null || true
        wait "$qemu_pid" 2>/dev/null || true
    fi
    rm -f "$qemu_pid_file" "$success_file"
}
trap cleanup EXIT

fail() {
    tail -n 160 "$output_file" >&2
    echo "Rocky systemd smoke test failed: $1" >&2
    stop_qemu
    exit 1
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

diagnose_login_timeout() {
    printf '\035'
    if wait_for "[detached from domain; use 'domain attach NAME' to return]" 10; then
        printf '%s\n' \
            'domain status rocky' \
            'domain list' \
            'ps'
        sleep 2
        printf '%s\n' shutdown
        wait_for 'GINUX_KERNEL_OK' 15 || true
    fi
    fail "timed out waiting for 'login:'"
}

wait_for_login() {
    local limit=${GINUX_ROCKY_LOGIN_TIMEOUT:-180}
    for ((attempt = 0; attempt < limit; attempt++)); do
        grep -Eq 'login:|\[root@ginux[^]]*\]#|bash-[0-9.]+#' "$output_file" && return 0
        if grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC|Linux init (execution|setup) failed|Linux init exited with status|rocky: (failed|stopped)' "$output_file"; then
            break
        fi
        sleep 1
    done
    diagnose_login_timeout
}

mkdir -p "$(dirname "$volume_image")"
: >"$output_file"
if [[ ! -f "$volume_image" ]]; then
    truncate -s "${GINUX_ROCKY_VOLUME_SIZE:-64G}" "$volume_image"
fi
{
    wait_for 'ginux> '
    domain_started_at=$(date +%s)
    printf '%s\n' \
        'domain new rocky' \
        "domain set rocky image=$rocky_image init=/sbin/init" \
        'domain start rocky --attach'
    wait_for 'ginux: OCI image ready' 240
    wait_for 'ginux: entering configured Linux init' 30
    init_started_at=$(date +%s)
    wait_for ' running in system mode (' 30
    wait_for_login
    if [[ "${GINUX_VERIFY_LOGIN_SHELL:-0}" == 1 ]]; then
        if ! grep -Eq '\[root@ginux[^]]*\]#|bash-[0-9.]+#' "$output_file"; then
            printf '%s\n' root
            sleep 2
            if grep -Fq 'Password:' "$output_file"; then
                fail 'Rocky root account requires a console password'
            fi
        fi
        printf '%s\n' "printf 'GINUX_LOGIN_%s\\n' SHELL_READY"
        wait_for 'GINUX_LOGIN_SHELL_READY' 15
    fi
    login_reached_at=$(date +%s)
    echo "Rocky timing domain-start-to-login: $((login_reached_at - domain_started_at)) s" >&2
    echo "Rocky timing Linux-init-to-login: $((login_reached_at - init_started_at)) s" >&2
    if [[ "${GINUX_REQUIRE_OCI_CACHE_HIT:-0}" == 1 ]] \
        && ! grep -Fq 'ginux: OCI cache hit (' "$output_file"; then
        fail 'warm boot did not restore the image from the OCI cache'
    fi
    if grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC|Linux init (execution|setup) failed|Linux init exited with status|Linux child exec rejected|systemd-system-update-generator failed|rocky: (failed|stopped)' "$output_file"; then
        fail 'systemd failed before the console login target'
    fi
    printf '%s\n' ok >"$success_file"
    stop_qemu
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
    && grep -Eq 'login:|\[root@ginux[^]]*\]#|bash-[0-9.]+#' "$output_file" \
    && ! grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC|Linux init (execution|setup) failed|Linux init exited with status|Linux child exec rejected|systemd-system-update-generator failed|rocky: (failed|stopped)' "$output_file"; then
    echo 'Rocky systemd console login target passed.'
    exit 0
fi

if ! grep -Fq 'GINUX_KERNEL_OK' "$output_file" \
    || grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC' "$output_file"; then
    fail 'clean-shutdown marker was not observed'
fi

fail 'systemd did not reach the console login target'
