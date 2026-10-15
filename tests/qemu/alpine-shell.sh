#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
output_file=$(mktemp "${TMPDIR:-/tmp}/ginux-alpine-shell.XXXXXX")
volume_image=$(mktemp "${TMPDIR:-/tmp}/ginux-alpine-shell-volume.XXXXXX")
qemu_pid_file=$(mktemp "${TMPDIR:-/tmp}/ginux-alpine-shell-pid.XXXXXX")
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
    for _ in {1..60}; do
        grep -Fq "$marker" "$output_file" && return 0
        if grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC' "$output_file"; then
            break
        fi
        sleep 1
    done
    tail -n 120 "$output_file" >&2
    echo "Alpine shell test failed while waiting for '$marker'." >&2
    stop_qemu
    return 1
}

wait_for_line() {
    local marker=$1
    for _ in {1..60}; do
        grep -Eq "^${marker}\\r?$" "$output_file" && return 0
        if grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC' "$output_file"; then
            break
        fi
        sleep 1
    done
    tail -n 120 "$output_file" >&2
    echo "Alpine shell test failed while waiting for line '$marker'." >&2
    stop_qemu
    return 1
}

wait_for_count() {
    local marker=$1
    local expected=$2
    for _ in {1..60}; do
        local observed
        observed=$(grep -Fc "$marker" "$output_file" || true)
        [[ "$observed" -ge "$expected" ]] && return 0
        if grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC' "$output_file"; then
            break
        fi
        sleep 1
    done
    tail -n 120 "$output_file" >&2
    echo "Alpine shell test failed while waiting for $expected occurrences of '$marker'." >&2
    stop_qemu
    return 1
}

dd if=/dev/zero of="$volume_image" bs=1048576 count=1 2>/dev/null
{
    wait_for 'ginux> '
    printf '%s\n' \
        'domain new alpine' \
        'domain set alpine personality=linux image=alpine:latest init=/bin/sh' \
        'domain list'
    wait_for 'docker.io/library/alpine:latest'
    printf '%s\n' \
        'domain start alpine --attach'
    wait_for 'ginux: entering configured OCI init'
    wait_for 'ginux: OCI image ready'
    printf '%s\n' \
        "printf 'LIBC_RUNTIME_OK\\n'" \
        "test -f /bin/busybox && printf 'STAT_ABI_OK\\n'" \
        "while IFS= read -r line; do echo \"\$line\"; done < /etc/os-release" \
        'echo /*' \
        'echo /etc/os-*' \
        'pwd' \
        "ls; printf 'ROOT_CWD_STATUS=%s\\n' \"\$?\"" \
        "cd /etc; ls .; printf 'RELATIVE_CWD_STATUS=%s\\n' \"\$?\"" \
        'pwd' \
        'cd /' \
        '/bin/ls /etc' \
        '/bin/cat /etc/os-release' \
        'ps aux' \
        'ps' \
        'ls /proc' \
        "test -f /proc/1/status && printf 'PROCFS_OK\\n'" \
        "test -f /proc/cpuinfo && test -f /proc/meminfo && test -f /proc/stat && test -f /proc/uptime && test -f /proc/loadavg && test -f /proc/filesystems && test -f /proc/cmdline && test -f /proc/self/stat && test -f /proc/self/statm && printf 'PROCFS_SYSTEM_OK\\n'" \
        '/bin/cat /proc/1/status' \
        '/bin/cat /proc/1/stat' \
        '/bin/cat /proc/1/statm' \
        '/bin/cat /proc/1/mountinfo' \
        'ls /' \
        "printf 'FORK_STATE_RESTORE_OK\\n'" \
        "printf 'FORK_EXEC_OK\\n'" \
        "printf 'OCI_FILESYSTEM_OK\\n'"
    wait_for_line 'LIBC_RUNTIME_OK'
    wait_for_line 'STAT_ABI_OK'
    wait_for_line 'NAME="Alpine Linux"'
    wait_for_line '/'
    wait_for_line 'ROOT_CWD_STATUS=0'
    wait_for_line 'RELATIVE_CWD_STATUS=0'
    wait_for_line '/etc'
    wait_for_line 'FORK_STATE_RESTORE_OK'
    wait_for_line 'FORK_EXEC_OK'
    wait_for_line 'PROCFS_OK'
    wait_for_line 'PROCFS_SYSTEM_OK'
    wait_for_line 'OCI_FILESYSTEM_OK'
    printf '\035'
    wait_for "[detached from domain; use 'domain attach NAME' to return]"
    wait_for_count 'ginux> ' 2
    printf '%s\n' 'domain list'
    wait_for 'alpine          running     linux'
    printf '%s\n' 'domain attach alpine'
    wait_for_count $'\033[2J\033[H' 1
    printf '%s\n' 'exit 0'
    wait_for_count 'ginux: OCI init exited cleanly' 1
    wait_for_count 'ginux> ' 3
    printf '%s\n' 'domain list'
    wait_for 'alpine          stopped     linux'
    wait_for_count 'ginux> ' 4
    printf '%s\n' 'domain start alpine --attach'
    wait_for_count 'ginux: entering configured OCI init' 2
    printf '%s\n' \
        "printf 'OCI_RESTART_OK\\n'" \
        '/bin/ls /etc/os-release' \
        'exit 0'
    wait_for_line 'OCI_RESTART_OK'
    wait_for_count 'ginux: OCI init exited cleanly' 2
    wait_for_count 'ginux> ' 5
    printf '%s\n' \
        'domain set alpine personality=linux init=/missing' \
        'domain start alpine --attach'
    wait_for 'ginux: OCI init execution failed; domain marked failed'
    wait_for_count 'ginux> ' 6
    printf '%s\n' 'domain status alpine'
    wait_for_line 'alpine: failed'
    wait_for_count 'ginux> ' 7
    printf '%s\n' 'shutdown'
} | GINUX_VOLUME_IMAGE="$volume_image" GINUX_QEMU_PID_FILE="$qemu_pid_file" \
    "$repo_root/tools/qemu/run.sh" >"$output_file" 2>&1 &
qemu_pid=$!

for _ in {1..300}; do
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
    tail -n 120 "$output_file"
    echo "Alpine shell test failed: QEMU did not shut down." >&2
    exit 1
fi
wait "$qemu_pid" || true
qemu_pid=""

for expected in \
    'ginux: entering configured OCI init' \
    'LIBC_RUNTIME_OK' \
    'STAT_ABI_OK' \
    'NAME="Alpine Linux"' \
    'ROOT_CWD_STATUS=0' \
    'RELATIVE_CWD_STATUS=0' \
    'FORK_STATE_RESTORE_OK' \
    '/etc' \
    'FORK_EXEC_OK' \
    'PROCFS_OK' \
    'PROCFS_SYSTEM_OK' \
    'OCI_FILESYSTEM_OK' \
    'OCI_RESTART_OK' \
    'ginux: OCI init execution failed; domain marked failed' \
    'alpine: failed' \
    'ginux: OCI init exited cleanly' \
    'GINUX_KERNEL_OK'; do
    if ! grep -Fq "$expected" "$output_file"; then
        tail -n 120 "$output_file"
        echo "Alpine shell test failed: missing '$expected'." >&2
        exit 1
    fi
done

if grep -Fq 'live registry refresh snapshot failed' "$output_file"; then
    tail -n 120 "$output_file"
    echo "Alpine shell test failed: forked applet corrupted the parent process registry." >&2
    exit 1
fi

if grep -Eq '^GINUX_(BOOT|KERNEL)_PANIC|ginux: unsupported Linux syscall' "$output_file"; then
    tail -n 120 "$output_file"
    echo "Alpine shell test failed: kernel panic or syscall debug noise detected." >&2
    exit 1
fi

if [[ $(grep -Fc 'ginux: entering configured OCI init' "$output_file") -ne 3 ]] \
    || [[ $(grep -Fc 'ginux: OCI init exited cleanly' "$output_file") -ne 2 ]]; then
    tail -n 120 "$output_file"
    echo "Alpine shell test failed: restart lifecycle counts were incorrect." >&2
    exit 1
fi

if ! grep -Eq '(^|[[:space:]])/bin([[:space:]]|$)' "$output_file" \
    || ! grep -Eq '(^|[[:space:]])/etc/os-release([[:space:]]|$)' "$output_file"; then
    tail -n 120 "$output_file"
    echo "Alpine shell test failed: OCI directory contents were not listed." >&2
    exit 1
fi

echo "Alpine OCI shell, restart, and failure-containment test passed."
