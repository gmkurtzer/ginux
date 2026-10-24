#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=tools/benchmarks/common.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

mkdir -p "$benchmark_dir"
volume="$benchmark_dir/ginux-volume.raw"
rootfs="$benchmark_dir/sysbench-minimal-rootfs.tar"

[[ -s "$rootfs" ]] || {
    echo "benchmark: missing $rootfs; prepare the local sysbench image first" >&2
    exit 1
}
sysbench_hash=$(tar -xOf "$rootfs" usr/bin/sysbench | {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum; else shasum -a 256; fi
} | awk '{print $1}')
sample_runs=$((BENCHMARK_BOOT_RUNS * BENCHMARK_WORKLOAD_RUNS))

for workload in cpu memory threads mutex; do
    for ((run = 1; run <= sample_runs; run++)); do
        log="$benchmark_dir/ginux-sysbench-${workload}-${run}.log"
        if grep -Fq "GINUX_BENCH_WORKLOAD_END_${workload}_1=0" "$log" 2>/dev/null \
            && grep -Fq "GINUX_BENCH_SYSBENCH_SHA256 $sysbench_hash" "$log"; then
            continue
        fi
        GINUX_ROCKY_LOG="$log" \
        GINUX_VOLUME_IMAGE="$volume" \
        GINUX_MEMORY="$BENCHMARK_MEMORY" \
        GINUX_ROCKY_IMAGE="$GINUX_SYSBENCH_IMAGE" \
        GINUX_ROCKY_INIT=/bin/sh \
        GINUX_REQUIRE_OCI_CACHE_HIT="${GINUX_REQUIRE_OCI_CACHE_HIT:-0}" \
        GINUX_BENCHMARK_SYSBENCH=1 \
        GINUX_BENCHMARK_SINGLE_RUN=1 \
        GINUX_BENCHMARK_WORKLOAD="$workload" \
        GINUX_BENCHMARK_SYSBENCH_SHA256="$sysbench_hash" \
        GINUX_BENCHMARK_WORKLOAD_RUNS=1 \
        GINUX_BENCHMARK_WORKLOAD_SECONDS="$BENCHMARK_WORKLOAD_SECONDS" \
        GINUX_ROCKY_RUNTIME_TIMEOUT=900 \
        "$benchmark_root/tests/qemu/rocky-systemd.sh"
    done
done

echo "Ginux benchmark completed: $benchmark_dir/ginux-sysbench-*.log"
