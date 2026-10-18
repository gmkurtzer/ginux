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
    'Interfaces: 17 pass, 5 fail, 1 policy-exempt, 23 total' \
    'Syscalls: 82 pass, 245 fail, 48 policy-exempt, 375 total' \
    'Linux userspace conformance results:' \
    'NAME/IDENTIFIER' \
    'Summary: 99 pass, 250 fail, 49 policy-exempt, 398 total' \
    'PASS = verified; FAIL = incomplete or unsupported; POLICY-EXEMPT = deliberately denied.' \
    '1      0      R      controller' \
    'Shutting down Ginux.'; do
    if ! grep -Fq "$expected" "$output_file"; then
        cat "$output_file"
        echo "Linux ABI report test failed: missing output: $expected" >&2
        exit 1
    fi
done

for expected_regex in \
    '^syscall-entry +interface +0 +PASS' \
    '^sysfs +interface +16 +FAIL' \
    '^linux-module-userspace-api +interface +22 +POLICY-EXEMPT' \
    '^read +syscall +0 +FAIL' \
    '^sched_yield +syscall +24 +PASS' \
    '^accept +syscall +43 +FAIL' \
    '^setuid +syscall +105 +POLICY-EXEMPT' \
    '^mseal +syscall +462 +FAIL' \
    '^9999 +syscall +- +FAIL'; do
    if ! grep -Eq "$expected_regex" "$output_file"; then
        cat "$output_file"
        echo "Linux ABI report test failed: missing row matching: $expected_regex" >&2
        exit 1
    fi
done

read -r report_rows interface_rows syscall_rows pass_rows fail_rows exempt_rows < <(
    awk '
        /Linux userspace conformance results:/ { in_report = 1; next }
        in_report && /^Summary:/ { exit }
        in_report && ($2 == "interface" || $2 == "syscall") {
            gsub(/\r/, "", $4)
            total++
            types[$2]++
            results[$4]++
        }
        END {
            print total + 0, types["interface"] + 0, types["syscall"] + 0,
                results["PASS"] + 0, results["FAIL"] + 0,
                results["POLICY-EXEMPT"] + 0
        }
    ' "$output_file"
)
if [[ "$report_rows $interface_rows $syscall_rows $pass_rows $fail_rows $exempt_rows" != \
    '398 23 375 99 250 49' ]]; then
    cat "$output_file"
    echo "Linux ABI report test failed: incomplete or inconsistent result table." >&2
    exit 1
fi

for rejected in GINUX_KERNEL_PANIC 'controller failed; restarting control plane' UNVERIFIED POLICY-DENIED UNSUPPORTED; do
    if grep -Fq "$rejected" "$output_file"; then
        cat "$output_file"
        echo "Linux ABI report test failed: observed: $rejected" >&2
        exit 1
    fi
done

echo "Linux ABI controller report passed (23 interfaces, 375 syscalls)."
