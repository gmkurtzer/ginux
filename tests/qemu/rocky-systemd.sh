#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
output_file="${GINUX_ROCKY_LOG:-$repo_root/build/qemu/rocky-systemd.log}"
volume_image="${GINUX_VOLUME_IMAGE:-$repo_root/build/qemu/rocky-systemd-volume.raw}"
qemu_pid_file=$(mktemp "${TMPDIR:-/tmp}/ginux-rocky-systemd-pid.XXXXXX")
success_file=$(mktemp "${TMPDIR:-/tmp}/ginux-rocky-systemd-success.XXXXXX")
qemu_pid=""
rocky_image="${GINUX_ROCKY_IMAGE:-rockylinux/rockylinux:10-ubi-init}"
start_mode="${GINUX_ROCKY_START_MODE:-attached}"

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
        if grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC|^ginux: Linux user fault|Linux init (execution|setup) failed|rocky: (failed|stopped)' "$output_file"; then
            break
        fi
        sleep 1
    done
    fail "timed out waiting for '$marker'"
}

wait_for_line() {
    local marker=$1
    local limit=${2:-30}
    for ((attempt = 0; attempt < limit; attempt++)); do
        grep -Eq "^${marker}\r?$" "$output_file" && return 0
        sleep 1
    done
    fail "timed out waiting for output line '$marker'"
}

wait_for_count() {
    local marker=$1
    local expected=$2
    local limit=${3:-30}
    for ((attempt = 0; attempt < limit; attempt++)); do
        if (( $(grep -Fc "$marker" "$output_file" || true) >= expected )); then
            return 0
        fi
        if grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC|^ginux: Linux user fault|Linux init (execution|setup) failed|rocky: (failed|stopped)' "$output_file"; then
            break
        fi
        sleep 1
    done
    fail "timed out waiting for occurrence $expected of '$marker'"
}

now_ms() {
    perl -MTime::HiRes=time -e 'printf "%.0f\n", time() * 1000'
}

wait_for_count_ms() {
    local marker=$1
    local expected=$2
    local limit_ms=$3
    local started
    local now
    started=$(now_ms)
    while true; do
        if (( $(grep -Fc "$marker" "$output_file" || true) >= expected )); then
            now=$(now_ms)
            printf '%s\n' "$((now - started))"
            return 0
        fi
        now=$(now_ms)
        if ((now - started >= limit_ms)); then
            return 1
        fi
        sleep 0.02
    done
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
        grep -Eq 'ginux login:' "$output_file" && return 0
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
        "domain set rocky image=$rocky_image init=/sbin/init"
    if [[ "$start_mode" == detached || "$start_mode" == detached-return || "$start_mode" == responsive || "$start_mode" == stop-restart ]]; then
        printf '%s\n' 'domain start rocky'
    else
        printf '%s\n' 'domain start rocky --attach'
    fi
    if [[ "$start_mode" == detached-return ]]; then
        wait_for 'rocky: starting (detached)' 10
        wait_for_count 'ginux> ' 2 10
        printf '%s\n' 'domain status rocky'
        wait_for 'State:       starting' 240
        printf '%s\n' 'domain list' 'domain status rocky'
        wait_for 'rocky           booting' 120
        wait_for 'State:       booting' 10
        wait_for 'Execution:   booting' 10
        if grep -Eq 'ginux: (checking OCI cache|OCI cache hit|OCI unpacking image|starting configured runtime domain|entering configured Linux init)' "$output_file"; then
            fail 'detached lifecycle output leaked onto the controller console'
        fi
        if grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC|^ginux: Linux user fault|Linux init (execution|setup) failed|rocky: (failed|stopped)' "$output_file"; then
            fail 'background domain did not return cleanly to the controller'
        fi
        printf '%s\n' ok >"$success_file"
        stop_qemu
        exit 0
    fi
    if [[ "$start_mode" == responsive ]]; then
        wait_for 'rocky: starting (detached)' 10
        wait_for_count 'ginux> ' 2 10
        # Advance cache restore and publication as separate background stages.
        sleep 1
        printf '%s\n' 'domain status rocky'
        wait_for 'State:       starting' 240
        sleep 0.1
        printf '%s\n' 'domain status rocky'
        wait_for 'State:       booting' 120
        # The first Linux timer handoff existed before this regression test.
        # Consume it, resume Linux, and measure the required recurring handoff.
        local_catalog_count=$(grep -Fc 'GINUX.EFI component catalog v0:' "$output_file" || true)
        sleep 0.1
        printf '%s\n' components
        wait_for_count 'GINUX.EFI component catalog v0:' "$((local_catalog_count + 1))" 10
        local_catalog_count=$((local_catalog_count + 1))
        sleep 0.1
        printf '%s\n' components
        if ! boot_response_ms=$(wait_for_count_ms 'GINUX.EFI component catalog v0:' "$((local_catalog_count + 1))" 1000); then
            fail 'controller did not respond within 1000 ms while detached Linux work was running'
        fi
        printf '%s\n' shutdown
        wait_for 'GINUX_KERNEL_OK' 15
        printf '%s\n' "$boot_response_ms" >"$success_file"
        stop_qemu
        exit 0
    fi
    if [[ "$start_mode" != detached && "$start_mode" != stop-restart ]]; then
        wait_for 'ginux: OCI image ready' 240
        wait_for 'ginux: entering configured Linux init' 30
    fi
    init_started_at=$(date +%s)
    if [[ "$start_mode" == stop-restart ]]; then
        for _ in {1..90}; do
            printf '%s\n' 'domain status rocky'
            sleep 1
            grep -Fq 'State:       running' "$output_file" && break
        done
        wait_for 'State:       running' 1
        printf '%s\n' 'domain stop rocky'
        wait_for 'rocky: stopped' 30
        sleep 1
        printf '%s\n' 'domain start rocky --attach'
        for _ in {1..30}; do
            (( $(grep -Fc 'ginux: entering configured Linux init' "$output_file" || true) >= 2 )) && break
            sleep 1
        done
        if (( $(grep -Fc 'ginux: entering configured Linux init' "$output_file" || true) < 2 )); then
            fail "restart did not enter Linux init"
        fi
        for _ in {1..90}; do
            grep -Fq 'ginux login:' "$output_file" && break
            sleep 1
        done
        grep -Fq 'ginux login:' "$output_file" || fail "restart did not reach login prompt"
    elif [[ "$start_mode" == detached ]]; then
        for _ in {1..90}; do
            printf '%s\n' 'domain status rocky'
            sleep 1
            grep -Fq 'State:       running' "$output_file" && break
        done
        wait_for 'State:       running' 1
        printf '%s\n' 'domain attach rocky'
    else
        wait_for ' running in system mode (' 30
    fi
    wait_for_login
    if ! grep -Eq '^ginux: timing Linux-init-to-console: [0-9]+ ms' "$output_file"; then
        fail 'kernel did not report Linux init timing'
    fi
    if [[ "$start_mode" != detached ]] \
        && ! grep -Eq '\[.*OK.*\].*Listening on .*systemd-initctl' "$output_file"; then
        fail 'systemd initctl named FIFO did not become active'
    fi
    if grep -Eq 'Unexpected error while writing to journal file|Failed to replace specifiers.*%b' "$output_file"; then
        fail 'systemd boot ID or journal file contract regressed'
    fi
    if grep -Fq 'FlushToVar() varlink call: Connection reset by peer' "$output_file"; then
        fail 'systemd journal varlink transport regressed'
    fi
    if grep -Eq '\[root@ginux[^]]*\]#|bash-[0-9.]+#' "$output_file"; then
        fail 'domain console bypassed login authentication'
    fi
    if grep -q 'FAILED.*Failed to start' "$output_file"; then
        fail 'systemd reported a failed boot unit'
    fi
    if grep -Fq 'ginux: Linux child exited with status' "$output_file"; then
        fail 'kernel leaked an anonymous userspace child exit'
    fi
    if [[ "${GINUX_TEST_DOMAIN_EXEC:-0}" == 1 ]]; then
        printf '\035'
        wait_for "[detached from domain; use 'domain attach NAME' to return]" 10
        wait_for_count 'ginux> ' 2 10
        printf '%s\n' 'domain exec rocky /bin/echo DOMAIN_EXEC_OK'
        wait_for 'DOMAIN_EXEC_OK' 30
        wait_for 'rocky: command completed' 30
        printf '%s\n' 'domain status rocky'
        wait_for 'State:       running' 10
        printf '%s\n' 'domain exec rocky --interactive /bin/sh'
        wait_for '# ' 30
        printf '%s\n' "printf '\\nDOMAIN_EXEC_INTERACTIVE_OK\\n'"
        wait_for_line 'DOMAIN_EXEC_INTERACTIVE_OK' 30
        printf '%s\n' 'exit'
        wait_for_count 'ginux> ' 3 30
    fi
    if [[ "$start_mode" == detached ]]; then
        printf '\035'
        wait_for '[detached from domain; use '\''domain attach NAME'\'' to return]' 10
        wait_for_count 'ginux> ' 3 10
        printf '%s\n' 'domain status rocky'
        sleep 1
        printf '%s\n' 'domain attach rocky'
        wait_for_count 'ginux login:' 1 10
    fi
    login_reached_at=$(date +%s)
    init_to_console_ms=$(grep -E '^ginux: timing Linux-init-to-console: [0-9]+ ms' "$output_file" | tail -n 1 | awk '{print $4}')
    echo "Rocky timing Linux-init-to-console: ${init_to_console_ms} ms" >&2
    echo "Rocky timing domain-start-to-login: $((login_reached_at - domain_started_at)) s" >&2
    echo "Rocky timing Linux-init-to-login: $((login_reached_at - init_started_at)) s" >&2
    if [[ -n "${GINUX_MAX_LINUX_INIT_MS:-}" ]] \
        && ((init_to_console_ms > GINUX_MAX_LINUX_INIT_MS)); then
        fail "Linux init exceeded ${GINUX_MAX_LINUX_INIT_MS} ms budget (${init_to_console_ms} ms)"
    fi
    if [[ "${GINUX_REQUIRE_OCI_CACHE_HIT:-0}" == 1 ]] \
        && ! grep -Fq 'ginux: OCI cache hit (' "$output_file"; then
        fail 'warm boot did not restore the image from the OCI cache'
    fi
    lifecycle_failure='rocky: (failed|stopped)'
    [[ "$start_mode" == stop-restart ]] && lifecycle_failure='rocky: failed'
    if grep -Eq "^GINUX_(BOOT|KERNEL)_PANIC|Linux init (execution|setup) failed|Linux init exited with status|Linux child exec rejected|systemd-system-update-generator failed|$lifecycle_failure" "$output_file"; then
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

if [[ "$start_mode" == detached-return && -s "$success_file" ]] \
    && grep -Fq 'rocky: starting (detached)' "$output_file" \
    && grep -Fq 'rocky           booting' "$output_file" \
    && grep -Fq 'State:       booting' "$output_file" \
    && ! grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC|^ginux: Linux user fault|Linux init (execution|setup) failed|rocky: (failed|stopped)' "$output_file"; then
    echo 'Rocky background start returned to the controller with observable boot state.'
    exit 0
fi

if [[ "$start_mode" == responsive && -s "$success_file" ]] \
    && grep -Fq 'GINUX_KERNEL_OK' "$output_file" \
    && ! grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC|^ginux: Linux user fault|Linux init (execution|setup) failed|rocky: (failed|stopped)' "$output_file"; then
    read -r boot_response_ms <"$success_file"
    echo "Controller remained responsive while detached Linux work was running (${boot_response_ms} ms)."
    exit 0
fi

final_lifecycle_failure='rocky: (failed|stopped)'
[[ "$start_mode" == stop-restart ]] && final_lifecycle_failure='rocky: failed'
if [[ -s "$success_file" ]] \
    && grep -Fq 'ginux login:' "$output_file" \
    && ! grep -Eq "^GINUX_(BOOT|KERNEL)_PANIC|Linux init (execution|setup) failed|Linux init exited with status|Linux child exec rejected|systemd-system-update-generator failed|$final_lifecycle_failure" "$output_file"; then
    echo 'Rocky systemd console login target passed.'
    exit 0
fi

if ! grep -Fq 'GINUX_KERNEL_OK' "$output_file" \
    || grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC' "$output_file"; then
    fail 'clean-shutdown marker was not observed'
fi

fail 'systemd did not reach the console login target'
