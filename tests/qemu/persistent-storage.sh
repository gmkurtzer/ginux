#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
test_dir="$(mktemp -d -t ginux-storage.XXXXXX)"
volume_image="$test_dir/volume.raw"
qemu_pid_file="$test_dir/qemu.pid"
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
    rm -rf "$test_dir"
}
trap cleanup EXIT

run_boot() {
    local output_file="$1"
    local commands="$2"
    : >"$qemu_pid_file"
    {
        for _ in {1..120}; do
            if grep -aFq 'ginux> ' "$output_file"; then
                printf '%s' "$commands"
                exit 0
            fi
            if grep -aFq 'GINUX_KERNEL_PANIC' "$output_file"; then
                stop_qemu
                exit 0
            fi
            sleep 1
        done
        stop_qemu
    } | GINUX_VOLUME_IMAGE="$volume_image" GINUX_VOLUME_SIZE=64M \
        GINUX_QEMU_PID_FILE="$qemu_pid_file" \
        "$repo_root/tools/qemu/run.sh" >"$output_file" 2>&1 &
    qemu_pid=$!

    for _ in {1..135}; do
        if ! kill -0 "$qemu_pid" 2>/dev/null; then
            break
        fi
        sleep 1
    done
    if kill -0 "$qemu_pid" 2>/dev/null; then
        stop_qemu
        wait "$qemu_pid" 2>/dev/null || true
        qemu_pid=""
        cat "$output_file"
        echo "Persistent storage test timed out." >&2
        exit 1
    fi
    if ! wait "$qemu_pid"; then
        qemu_pid=""
        cat "$output_file"
        echo "Persistent storage boot failed." >&2
        exit 1
    fi
    qemu_pid=""
}

first_output="$test_dir/first.log"
second_output="$test_dir/second.log"
reset_output="$test_dir/reset.log"

run_boot "$first_output" $'network new gnet0 type=virtual\ndomain new persisted\ndomain set persisted image=alpine:latest init=/bin/sh cpu=2 memory-mib=256 autostart=true network=gnet0\nshutdown\n'
grep -aFq 'ginux: persistent storage initialized (generation 1, 131064 data sectors)' "$first_output"
grep -aEq '^ginux: timing boot-to-controller: [0-9]+ ms' "$first_output"
grep -aFq 'ginux: local configuration committed (generation 1)' "$first_output"

run_boot "$second_output" $'domain list\nnetwork list\nshutdown\n'
grep -aFq 'ginux: persistent storage recovered (generation 1, 131064 data sectors)' "$second_output"
grep -aFq 'persisted       configured' "$second_output"
grep -aFq '/bin/sh' "$second_output"
grep -aFq 'docker.io/library/alpine:latest' "$second_output"
grep -aFq 'gnet0' "$second_output"

rm -f "$volume_image"
run_boot "$reset_output" $'domain list\nshutdown\n'
grep -aFq 'ginux: persistent storage initialized (generation 1, 131064 data sectors)' "$reset_output"
if grep -aFq 'persisted       configured' "$reset_output"; then
    echo "Persistent storage reset retained a deleted domain configuration." >&2
    exit 1
fi

echo "Persistent storage and local configuration recovery/reset boots passed."
