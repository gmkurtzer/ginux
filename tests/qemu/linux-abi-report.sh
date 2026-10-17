#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output_file="$(mktemp -t ginux-linux-abi.XXXXXX)"
volume_image="$(mktemp -t ginux-linux-abi-volume.XXXXXX)"
qemu_pid_file="$(mktemp -t ginux-linux-abi-pid.XXXXXX)"
qemu_pid=""
: >"$qemu_pid_file"

cleanup() {
    local pid=""
    if [[ -s "$qemu_pid_file" ]]; then
        read -r pid <"$qemu_pid_file" || true
    fi
    if [[ "$pid" =~ ^[0-9]+$ ]] && kill -0 "$pid" 2>/dev/null; then
        kill "$pid" 2>/dev/null || true
    fi
    if [[ -n "$qemu_pid" ]] && kill -0 "$qemu_pid" 2>/dev/null; then
        kill "$qemu_pid" 2>/dev/null || true
        wait "$qemu_pid" 2>/dev/null || true
    fi
    rm -f "$output_file" "$volume_image" "$qemu_pid_file"
}
trap cleanup EXIT

dd if=/dev/zero of="$volume_image" bs=1048576 count=1 2>/dev/null
{
    prompt_count=1
    for _ in {1..60}; do
        [[ "$(grep -Fo 'ginux> ' "$output_file" | wc -l | tr -d ' ')" -ge "$prompt_count" ]] && break
        grep -Fq 'GINUX_KERNEL_PANIC' "$output_file" && break
        sleep 1
    done
    for command in 'test linux' 'test linux all' 'test linux syscall read' \
        'test linux syscall accept' 'test linux syscall 9999' ps; do
        printf '%s\n' "$command"
        prompt_count=$((prompt_count + 1))
        for _ in {1..60}; do
            [[ "$(grep -Fo 'ginux> ' "$output_file" | wc -l | tr -d ' ')" -ge "$prompt_count" ]] && break
            grep -Fq 'GINUX_KERNEL_PANIC' "$output_file" && break
            sleep 1
        done
    done
    printf 'shutdown\n'
} | GINUX_VOLUME_IMAGE="$volume_image" GINUX_QEMU_PID_FILE="$qemu_pid_file" \
    "$repo_root/tools/qemu/run.sh" >"$output_file" 2>&1 &
qemu_pid=$!

for _ in {1..75}; do
    grep -Fq 'GINUX_KERNEL_PANIC' "$output_file" && break
    ! kill -0 "$qemu_pid" 2>/dev/null && break
    sleep 1
done

if kill -0 "$qemu_pid" 2>/dev/null; then
    cat "$output_file"
    echo "Linux ABI report test failed: QEMU did not shut down." >&2
    exit 1
fi
if wait "$qemu_pid"; then
    :
else
    status=$?
    cat "$output_file"
    echo "Linux ABI report test failed: QEMU exited with $status." >&2
    exit 1
fi
qemu_pid=""
: >"$qemu_pid_file"

for expected in \
    'Linux userspace runtime conformance summary:' \
    'Interfaces: 17 passed, 6 failed, 23 total' \
    'Syscalls: 19 passed, 261 unverified, 44 policy-denied, 51 unsupported, 375 total' \
    'Linux userspace interface runtime tests (bounded profile):' \
    'PASS syscall-entry' \
    'PASS socket-api' \
    'FAIL linux-module-userspace-api' \
    'UNVERIFIED 0 read' \
    'UNVERIFIED 34 pause' \
    'UNVERIFIED 37 alarm' \
    'UNVERIFIED 40 sendfile' \
    'UNVERIFIED 43 accept' \
    'UNVERIFIED 78 getdents' \
    'UNVERIFIED 49 bind' \
    'UNVERIFIED 50 listen' \
    'UNVERIFIED 109 setpgid' \
    'UNVERIFIED 112 setsid' \
    'UNVERIFIED 130 rt_sigsuspend' \
    'UNVERIFIED 219 restart_syscall' \
    'UNVERIFIED 247 waitid' \
    'UNVERIFIED 285 fallocate' \
    'UNVERIFIED 288 accept4' \
    'UNVERIFIED 326 copy_file_range' \
    'UNVERIFIED 437 openat2' \
    'PASS 134 uselib' \
    'PASS 136 ustat' \
    'PASS 139 sysfs' \
    'PASS 156 _sysctl' \
    'PASS 174 create_module' \
    'PASS 212 lookup_dcookie' \
    'PASS 236 vserver' \
    'POLICY-DENIED 105 setuid' \
    'POLICY-DENIED 164 settimeofday' \
    'POLICY-DENIED 305 clock_adjtime' \
    'UNVERIFIED 462 mseal' \
    'Summary: 19 passed, 261 unverified, 44 policy-denied, 51 unsupported, 375 total' \
    'FAIL unknown Linux syscall' \
    '1      0      R      controller' \
    'Shutting down Ginux.'; do
    if ! grep -Fq "$expected" "$output_file"; then
        cat "$output_file"
        echo "Linux ABI report test failed: missing output: $expected" >&2
        exit 1
    fi
done

for rejected in GINUX_KERNEL_PANIC 'controller failed; restarting control plane'; do
    if grep -Fq "$rejected" "$output_file"; then
        cat "$output_file"
        echo "Linux ABI report test failed: observed: $rejected" >&2
        exit 1
    fi
done

echo "Linux ABI controller report passed (23 interfaces, 375 syscalls)."
