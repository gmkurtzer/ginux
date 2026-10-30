#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tools/benchmarks/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

mkdir -p "$benchmark_dir"
volume="$benchmark_asset_dir/ginux-volume.raw"
rootfs="$benchmark_asset_dir/sysbench-minimal-rootfs.tar"

[[ -s "$rootfs" ]] || {
    echo "benchmark: missing $rootfs; prepare the local sysbench image first" >&2
    exit 1
}
sysbench_hash=$(tar -xOf "$rootfs" usr/bin/sysbench | {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum; else shasum -a 256; fi
} | awk '{print $1}')
if [[ "${GINUX_BENCHMARK_BATCHED:-0}" == 1 ]]; then
    for ((boot_run = 1; boot_run <= BENCHMARK_BOOT_RUNS; boot_run++)); do
        log="$benchmark_dir/ginux-sysbench-batch-$boot_run.log"
        GINUX_ROCKY_LOG="$log" \
        GINUX_VOLUME_IMAGE="$volume" \
        GINUX_MEMORY="$BENCHMARK_MEMORY" \
        GINUX_CPUS="$BENCHMARK_VCPUS" \
        GINUX_ROCKY_IMAGE="$GINUX_SYSBENCH_IMAGE" \
        GINUX_ROCKY_INIT=/bin/sh \
        GINUX_REQUIRE_OCI_CACHE_HIT=1 \
        GINUX_BENCHMARK_SYSBENCH=1 \
        GINUX_BENCHMARK_WORKLOAD="${GINUX_BENCHMARK_WORKLOADS:-cpu memory threads mutex}" \
        GINUX_BENCHMARK_THREADS="$BENCHMARK_THREADS" \
        GINUX_BENCHMARK_SYSBENCH_SHA256="$sysbench_hash" \
        GINUX_BENCHMARK_WORKLOAD_RUNS="$BENCHMARK_WORKLOAD_RUNS" \
        GINUX_BENCHMARK_WORKLOAD_SECONDS="$BENCHMARK_WORKLOAD_SECONDS" \
        GINUX_ROCKY_RUNTIME_TIMEOUT=1800 \
        "$benchmark_root/tests/qemu/rocky-systemd.sh"
    done
    echo "Ginux batched benchmark completed: $benchmark_dir/ginux-sysbench-batch-*.log"
    exit 0
fi

sample_runs=$((BENCHMARK_BOOT_RUNS * BENCHMARK_WORKLOAD_RUNS))

for workload in ${GINUX_BENCHMARK_WORKLOADS:-cpu memory threads mutex}; do
    for ((run = 1; run <= sample_runs; run++)); do
        log="$benchmark_dir/ginux-sysbench-${workload}-${run}.log"
        if [[ "${GINUX_BENCHMARK_FORCE:-0}" != 1 ]] \
            && grep -Fq "GINUX_BENCH_WORKLOAD_END_${workload}_1=0" "$log" 2>/dev/null \
            && grep -Fq "GINUX_BENCH_SYSBENCH_SHA256 $sysbench_hash" "$log"; then
            continue
        fi
        GINUX_ROCKY_LOG="$log" \
        GINUX_VOLUME_IMAGE="$volume" \
        GINUX_MEMORY="$BENCHMARK_MEMORY" \
        GINUX_CPUS="$BENCHMARK_VCPUS" \
        GINUX_ROCKY_IMAGE="$GINUX_SYSBENCH_IMAGE" \
        GINUX_ROCKY_INIT=/bin/sh \
        GINUX_REQUIRE_OCI_CACHE_HIT="${GINUX_REQUIRE_OCI_CACHE_HIT:-0}" \
        GINUX_BENCHMARK_SYSBENCH=1 \
        GINUX_BENCHMARK_SINGLE_RUN=1 \
        GINUX_BENCHMARK_WORKLOAD="$workload" \
        GINUX_BENCHMARK_THREADS="${GINUX_BENCHMARK_THREADS:-$BENCHMARK_THREADS}" \
        GINUX_BENCHMARK_SYSBENCH_SHA256="$sysbench_hash" \
        GINUX_BENCHMARK_WORKLOAD_RUNS=1 \
        GINUX_BENCHMARK_WORKLOAD_SECONDS="$BENCHMARK_WORKLOAD_SECONDS" \
        GINUX_ROCKY_RUNTIME_TIMEOUT=900 \
        "$benchmark_root/tests/qemu/rocky-systemd.sh"
    done
done

echo "Ginux benchmark completed: $benchmark_dir/ginux-sysbench-*.log"
