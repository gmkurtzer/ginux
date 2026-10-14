#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
output_file=$(mktemp "${TMPDIR:-/tmp}/ginux-alpine-shell.XXXXXX")
volume_image=$(mktemp "${TMPDIR:-/tmp}/ginux-alpine-shell-volume.XXXXXX")
qemu_pid=""

cleanup() {
    if [[ -n "$qemu_pid" ]] && kill -0 "$qemu_pid" 2>/dev/null; then
        kill "$qemu_pid" 2>/dev/null || true
        wait "$qemu_pid" 2>/dev/null || true
    fi
    rm -f "$output_file" "$volume_image"
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
    return 1
}

dd if=/dev/zero of="$volume_image" bs=1048576 count=1 2>/dev/null
{
    wait_for 'ginux> '
    printf '%s\n' \
        'container new alpine' \
        'container set alpine image=alpine:latest init=/bin/sh' \
        'container list'
    wait_for 'docker.io/library/alpine:3.24.2'
    printf '%s\n' \
        'container start alpine --attach'
    wait_for 'ginux: entering configured OCI init'
    printf '%s\n' \
        "printf 'LIBC_RUNTIME_OK\\n'" \
        "test -f /bin/busybox && printf 'STAT_ABI_OK\\n'" \
        "while IFS= read -r line; do echo \"\$line\"; done < /etc/os-release" \
        'echo /*' \
        'echo /etc/os-*' \
        'pwd' \
        '/bin/ls /etc' \
        '/bin/cat /etc/os-release' \
        "printf 'FORK_EXEC_OK\\n'" \
        "printf 'OCI_FILESYSTEM_OK\\n'"
    wait_for_line 'LIBC_RUNTIME_OK'
    wait_for_line 'STAT_ABI_OK'
    wait_for_line 'NAME="Alpine Linux"'
    wait_for_line '/'
    wait_for_line 'FORK_EXEC_OK'
    wait_for_line 'OCI_FILESYSTEM_OK'
    printf '%s\n' 'exit 0'
    wait_for 'ginux: OCI init exited cleanly'
    wait_for 'ginux> '
    printf '%s\n' 'shutdown'
} | GINUX_VOLUME_IMAGE="$volume_image" "$repo_root/tools/qemu/run.sh" >"$output_file" 2>&1 &
qemu_pid=$!

for _ in {1..90}; do
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
    'FORK_EXEC_OK' \
    'OCI_FILESYSTEM_OK' \
    'ginux: OCI init exited cleanly' \
    'GINUX_KERNEL_OK'; do
    if ! grep -Fq "$expected" "$output_file"; then
        tail -n 120 "$output_file"
        echo "Alpine shell test failed: missing '$expected'." >&2
        exit 1
    fi
done

if ! grep -Eq '(^|[[:space:]])/bin([[:space:]]|$)' "$output_file" \
    || ! grep -Eq '(^|[[:space:]])/etc/os-release([[:space:]]|$)' "$output_file"; then
    tail -n 120 "$output_file"
    echo "Alpine shell test failed: OCI directory contents were not listed." >&2
    exit 1
fi

echo "Alpine OCI shell and filesystem test passed."
