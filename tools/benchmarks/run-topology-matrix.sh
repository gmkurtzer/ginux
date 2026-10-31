#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tools/benchmarks/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

matrix_root="${GINUX_BENCHMARK_MATRIX_DIR:-$benchmark_root/build/benchmark/rocky10-topology-matrix}"
asset_dir="${GINUX_BENCHMARK_ASSET_DIR:-$benchmark_root/build/benchmark/rocky10-single-vcpu}"
force="${GINUX_BENCHMARK_FORCE:-0}"

mkdir -p "$matrix_root" "$asset_dir"

if [[ ! -s "$asset_dir/rocky-linux-prepared-v2.qcow2" ]]; then
    GINUX_BENCHMARK_DIR="$asset_dir" \
    GINUX_BENCHMARK_ASSET_DIR="$asset_dir" \
    BENCHMARK_VCPUS=1 BENCHMARK_THREADS=1 \
        "$benchmark_root/tools/benchmarks/prepare-rocky-linux.sh"
fi

if [[ ! -s "$asset_dir/sysbench-minimal-rootfs.tar" || ! -s "$asset_dir/ginux-volume.raw" ]]; then
    GINUX_BENCHMARK_DIR="$asset_dir" \
    GINUX_BENCHMARK_ASSET_DIR="$asset_dir" \
        "$benchmark_root/tools/benchmarks/prepare-ginux-sysbench.sh"
fi

run_configuration() {
    local mode=$1
    local cpus=$2
    local threads=$3
    local include_boot=$4
    local result_dir="$matrix_root/${mode}-${cpus}cpu-${threads}thread"
    local complete="$result_dir/.complete"

    if [[ "$force" != 1 && -s "$complete" ]]; then
        echo "Topology benchmark already complete: $mode, $cpus vCPU(s), $threads thread(s)"
        return 0
    fi

    mkdir -p "$result_dir"
    echo "Benchmarking $mode mode: $cpus vCPU(s), $threads thread(s)"

    if [[ "$force" == 1 || ! -s "$result_dir/.linux-complete" ]]; then
        GINUX_BENCHMARK_DIR="$result_dir" \
        GINUX_BENCHMARK_ASSET_DIR="$asset_dir" \
        BENCHMARK_VCPUS="$cpus" BENCHMARK_THREADS="$threads" \
            "$benchmark_root/tools/benchmarks/run-rocky-linux.sh"
        printf 'complete\n' >"$result_dir/.linux-complete"
    fi

    if [[ "$include_boot" == 1 && ( "$force" == 1 || ! -s "$result_dir/.ginux-boot-complete" ) ]]; then
        GINUX_BENCHMARK_DIR="$result_dir" \
        GINUX_BENCHMARK_ASSET_DIR="$asset_dir" \
        BENCHMARK_VCPUS="$cpus" BENCHMARK_THREADS="$threads" \
            "$benchmark_root/tools/benchmarks/run-ginux-boot.sh"
        GINUX_BENCHMARK_DIR="$result_dir" \
        GINUX_BENCHMARK_ASSET_DIR="$asset_dir" \
        BENCHMARK_VCPUS="$cpus" BENCHMARK_THREADS="$threads" \
            "$benchmark_root/tools/benchmarks/summarize-boot.sh" >"$result_dir/boot-summary.txt"
        printf 'complete\n' >"$result_dir/.ginux-boot-complete"
    fi

    if [[ "$force" == 1 || ! -s "$result_dir/.ginux-sysbench-complete" ]]; then
        GINUX_BENCHMARK_DIR="$result_dir" \
        GINUX_BENCHMARK_ASSET_DIR="$asset_dir" \
        BENCHMARK_VCPUS="$cpus" BENCHMARK_THREADS="$threads" \
        GINUX_BENCHMARK_BATCHED=1 \
            "$benchmark_root/tools/benchmarks/run-ginux.sh"
        printf 'complete\n' >"$result_dir/.ginux-sysbench-complete"
    fi

    GINUX_BENCHMARK_DIR="$result_dir" \
    GINUX_BENCHMARK_ASSET_DIR="$asset_dir" \
    BENCHMARK_VCPUS="$cpus" BENCHMARK_THREADS="$threads" \
        "$benchmark_root/tools/benchmarks/compare.sh" >"$result_dir/comparison.txt"

    printf 'mode\t%s\nvcpus\t%s\nthreads\t%s\n' "$mode" "$cpus" "$threads" \
        >"$result_dir/configuration.tsv"
    printf 'complete\n' >"$complete"
}

# The 1-vCPU/1-thread result is the common origin of both series.
run_configuration fixed 1 1 1
run_configuration fixed 2 1 1
run_configuration fixed 4 1 1
run_configuration scaled 2 2 0
run_configuration scaled 4 4 0

GINUX_BENCHMARK_MATRIX_DIR="$matrix_root" \
    "$benchmark_root/tools/benchmarks/summarize-topology-matrix.py"

if command -v lsof >/dev/null 2>&1; then
    if lsof "$asset_dir/ginux-volume.raw" >"$matrix_root/open-volume-check.log" 2>&1; then
        cat "$matrix_root/open-volume-check.log" >&2
        echo "benchmark: Ginux benchmark volume remains open" >&2
        exit 1
    fi
fi

echo "Topology matrix completed: $matrix_root"
