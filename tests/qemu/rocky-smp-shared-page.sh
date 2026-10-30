#!/usr/bin/env bash

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
result_root=${GINUX_SMP_PAGE_TEST_DIR:-$repo_root/build/benchmark/rocky-smp-shared-page}
asset_dir=${GINUX_BENCHMARK_ASSET_DIR:-$repo_root/build/benchmark/rocky10-single-vcpu}

for cpus in ${GINUX_SMP_PAGE_CPUS:-2 4}; do
    result_dir=$result_root/${cpus}cpu
    mkdir -p "$result_dir"

    GINUX_BENCHMARK_DIR="$result_dir" \
    GINUX_BENCHMARK_ASSET_DIR="$asset_dir" \
    BENCHMARK_VCPUS="$cpus" \
    BENCHMARK_THREADS="$cpus" \
    BENCHMARK_BOOT_RUNS=3 \
    BENCHMARK_WORKLOAD_RUNS=1 \
    BENCHMARK_WORKLOAD_SECONDS=3 \
    GINUX_BENCHMARK_BATCHED=1 \
    GINUX_BENCHMARK_WORKLOADS='cpu memory' \
        "$repo_root/tools/benchmarks/run-ginux.sh"

    for boot_run in 1 2 3; do
        log=$result_dir/ginux-sysbench-batch-${boot_run}.log
        grep -Fq 'GINUX_BENCH_WORKLOAD_END_cpu_1=0' "$log"
        grep -Fq 'GINUX_BENCH_WORKLOAD_END_memory_1=0' "$log"
        if grep -Eq 'Linux user fault|GINUX_(BOOT|KERNEL)_PANIC' "$log"; then
            echo "rocky-smp-shared-page: ${cpus}-vCPU fault or panic" >&2
            exit 1
        fi
    done
done

echo 'Rocky 2/4-vCPU shared-writable-page regression passed.'
