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
prepared_image="$benchmark_asset_dir/rocky-linux-prepared-v2.qcow2"
summary="$benchmark_dir/rocky-linux-summary.tsv"
qemu_pid=""

cleanup() {
    stop_process "$qemu_pid"
    if [[ "$qemu_pid" =~ ^[0-9]+$ ]]; then
        wait "$qemu_pid" 2>/dev/null || true
    fi
}
trap cleanup EXIT

mkdir -p "$benchmark_dir"
printf 'run\tqemu_start_to_multi_user_ms\taccelerator\tcpu\tvcpus\tthreads\tmemory\n' >"$summary"

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
        -fw_cfg "name=opt/ginux/threads,string=$BENCHMARK_THREADS" \
        -net none >"$log" 2>&1 &
    qemu_pid=$!
    ready=""
    for _ in {1..9000}; do
        if grep -Eq 'Reached target .*multi-user\.target' "$log"; then
            ready=$(now_ms)
            break
        fi
        kill -0 "$qemu_pid" 2>/dev/null || break
        sleep 0.02
    done
    if [[ -z "$ready" ]]; then
        stop_process "$qemu_pid"
        wait "$qemu_pid" 2>/dev/null || true
        tail -n 120 "$log" >&2
        echo "benchmark: Linux run $run did not reach multi-user.target" >&2
        exit 1
    fi
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$run" "$((ready - started))" "$accelerator" "$cpu_model" \
        "$BENCHMARK_VCPUS" "$BENCHMARK_THREADS" "$BENCHMARK_MEMORY" >>"$summary"
    if [[ "${BENCHMARK_BOOT_ONLY:-0}" == 1 ]]; then
        stop_process "$qemu_pid"
        wait "$qemu_pid" 2>/dev/null || true
        qemu_pid=""
        continue
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
    grep -Fq "GINUX_BENCH_THREADS $BENCHMARK_THREADS" "$results" || {
        cat "$results" >&2
        echo "benchmark: Linux run $run did not use $BENCHMARK_THREADS threads" >&2
        exit 1
    }
done

echo "Rocky Linux benchmark completed: $summary"
