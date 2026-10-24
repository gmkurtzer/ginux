#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tools/benchmarks/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

"$benchmark_root/tools/benchmarks/prepare-rocky-linux.sh"

qemu=$(find_qemu)
ovmf_code=$(find_ovmf_code)
ovmf_vars_template=$(find_ovmf_vars)
accelerator=$(benchmark_accelerator)
cpu_model=$(benchmark_cpu "$accelerator")
prepared_image="$benchmark_dir/rocky-linux-prepared.qcow2"
summary="$benchmark_dir/rocky-linux-summary.tsv"
qemu_pid=""

cleanup() {
    stop_process "$qemu_pid"
    if [[ "$qemu_pid" =~ ^[0-9]+$ ]]; then
        wait "$qemu_pid" 2>/dev/null || true
    fi
}
trap cleanup EXIT

printf 'run\tqemu_start_to_ready_ms\taccelerator\tcpu\tvcpus\tmemory\n' >"$summary"

for ((run = 1; run <= BENCHMARK_BOOT_RUNS; run++)); do
    overlay="$benchmark_dir/rocky-linux-run-$run.qcow2"
    vars="$benchmark_dir/rocky-linux-run-$run-vars.fd"
    log="$benchmark_dir/rocky-linux-run-$run.log"
    results="$benchmark_dir/rocky-linux-run-$run-results.log"
    rm -f "$overlay"
    qemu-img create -q -f qcow2 -F qcow2 -b "$prepared_image" "$overlay"
    cp "$ovmf_vars_template" "$vars"
    : >"$log"
    : >"$results"
    started=$(now_ms)
    "$qemu" \
        -machine "$BENCHMARK_MACHINE,accel=$accelerator" \
        -cpu "$cpu_model" -smp "$BENCHMARK_VCPUS" -m "$BENCHMARK_MEMORY" \
        -display none -monitor none -serial stdio -no-reboot \
        -drive "if=pflash,format=raw,readonly=on,file=$ovmf_code" \
        -drive "if=pflash,format=raw,file=$vars" \
        -drive "if=none,id=linux-root,format=qcow2,file=$overlay" \
        -device "virtio-blk-pci,drive=linux-root,bootindex=1" \
        -device virtio-serial \
        -chardev "file,id=benchmark-output,path=$results" \
        -device "virtserialport,chardev=benchmark-output,name=org.ginux.benchmark" \
        -net none >"$log" 2>&1 &
    qemu_pid=$!
    ready=""
    for _ in {1..1800}; do
        if grep -Fq GINUX_BENCH_READY "$results"; then
            ready=$(now_ms)
            break
        fi
        kill -0 "$qemu_pid" 2>/dev/null || break
        sleep 0.1
    done
    if [[ -z "$ready" ]]; then
        stop_process "$qemu_pid"
        wait "$qemu_pid" 2>/dev/null || true
        tail -n 120 "$log" >&2
        echo "benchmark: Linux run $run did not reach the ready marker" >&2
        exit 1
    fi
    for _ in {1..900}; do
        grep -Fq GINUX_BENCH_COMPLETE "$results" && break
        kill -0 "$qemu_pid" 2>/dev/null || break
        sleep 1
    done
    wait "$qemu_pid" || true
    qemu_pid=""
    grep -Fq GINUX_BENCH_COMPLETE "$results" || {
        cat "$results" >&2
        tail -n 120 "$log" >&2
        echo "benchmark: Linux workload run $run did not complete" >&2
        exit 1
    }
    printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$run" "$((ready - started))" "$accelerator" "$cpu_model" \
        "$BENCHMARK_VCPUS" "$BENCHMARK_MEMORY" >>"$summary"
done

echo "Rocky Linux benchmark completed: $summary"
