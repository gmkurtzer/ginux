#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
test_dir=$(mktemp -d "${TMPDIR:-/tmp}/ginux-oci-cache.XXXXXX")
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

require_marker() {
    local output_file=$1
    local marker=$2
    if ! grep -aEq "$marker" "$output_file"; then
        tail -n 160 "$output_file" >&2
        echo "OCI cache test missing expected output: $marker" >&2
        return 1
    fi
}

run_boot() {
    local output_file=$1
    local cached=$2
    : >"$qemu_pid_file"
    {
        for _ in {1..120}; do
            grep -aFq 'ginux> ' "$output_file" && break
            grep -aEq '^GINUX_(BOOT|KERNEL)_PANIC' "$output_file" && exit 0
            sleep 1
        done
        if [[ "$cached" == false ]]; then
            printf '%s\n' \
                'domain new alpine' \
                'domain set alpine image=alpine:latest init=/bin/sh' \
                'domain start alpine'
            for _ in {1..240}; do
                if grep -aFq 'ginux: OCI cache stored 1 layer' "$output_file" \
                    && grep -aFq 'ginux: local configuration committed' "$output_file"; then
                    break
                fi
                grep -aFq 'alpine: failed' "$output_file" && break
                grep -aEq '^GINUX_(BOOT|KERNEL)_PANIC' "$output_file" && exit 0
                sleep 1
            done
        else
            printf '%s\n' 'domain start alpine'
            for _ in {1..120}; do
                if grep -aFq 'ginux: OCI cache hit (1 layer)' "$output_file" \
                    && grep -aFq 'ginux: OCI image ready' "$output_file"; then
                    break
                fi
                grep -aFq 'alpine: failed' "$output_file" && break
                grep -aEq '^GINUX_(BOOT|KERNEL)_PANIC' "$output_file" && exit 0
                sleep 1
            done
        fi
        stop_qemu
    } | GINUX_VOLUME_IMAGE="$volume_image" GINUX_VOLUME_SIZE=64M \
        GINUX_QEMU_PID_FILE="$qemu_pid_file" \
        "$repo_root/tools/qemu/run.sh" >"$output_file" 2>&1 &
    qemu_pid=$!

    for _ in {1..360}; do
        ! kill -0 "$qemu_pid" 2>/dev/null && break
        sleep 1
    done
    if kill -0 "$qemu_pid" 2>/dev/null; then
        stop_qemu
        wait "$qemu_pid" 2>/dev/null || true
        qemu_pid=""
        tail -n 160 "$output_file"
        echo "OCI cache test timed out." >&2
        exit 1
    fi
    wait "$qemu_pid" || true
    qemu_pid=""
    if grep -aEq '^GINUX_(BOOT|KERNEL)_PANIC' "$output_file"; then
        tail -n 160 "$output_file"
        echo "OCI cache test encountered a kernel panic." >&2
        exit 1
    fi
}

cold_output="$test_dir/cold.log"
warm_output="$test_dir/warm.log"

run_boot "$cold_output" false
require_marker "$cold_output" 'ginux: OCI cache miss; pulling image'
require_marker "$cold_output" 'ginux: OCI cache stored 1 layer \([0-9]+ bytes\)'
require_marker "$cold_output" 'ginux: OCI image ready'

run_boot "$warm_output" true
require_marker "$warm_output" 'ginux: persistent storage recovered'
require_marker "$warm_output" 'ginux: OCI cache hit \(1 layer\)'
require_marker "$warm_output" 'ginux: OCI image ready'
if grep -aFq 'ginux: OCI authenticating with registry' "$warm_output" \
    || grep -aFq 'ginux: OCI layer 1/1: pulling' "$warm_output"; then
    tail -n 160 "$warm_output"
    echo "Warm OCI cache boot contacted the registry." >&2
    exit 1
fi

echo "Cold OCI pull and rebooted verified-layer cache hit passed."
